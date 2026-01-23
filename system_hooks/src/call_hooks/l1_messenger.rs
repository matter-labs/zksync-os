//!
//! L1 messenger system hook implementation.
//! This system hook is called by the L1 messenger system contract to send messages to L1.
//!
//! Note: By design this system hook should be indistinguishable from a call
//! to an empty account (for EVM compatibility reasons).
//!
use super::super::*;
use crate::addresses_constants::{L1_MESSENGER_ADDRESS, L1_MESSENGER_ADDRESS_HOOK};
use core::fmt::Write;
use ruint::aliases::{B160, U256};
use zk_ee::system_log;
use zk_ee::{
    execution_environment_type::ExecutionEnvironmentType,
    internal_error,
    system::{
        errors::{runtime::RuntimeError, system::SystemError},
        CallModifier, CompletedExecution, ExternalCallRequest,
    },
};

pub fn l1_messenger_hook<'a, S: EthereumLikeTypes>(
    request: ExternalCallRequest<S>,
    caller_ee: u8,
    system: &mut System<S>,
    return_memory: &'a mut [MaybeUninit<u8>],
) -> Result<(CompletedExecution<'a, S>, &'a mut [MaybeUninit<u8>]), SystemError>
where
{
    let ExternalCallRequest {
        available_resources,
        ergs_to_pass: _,
        input: calldata,
        call_scratch_space: _,
        nominal_token_value,
        caller,
        callee,
        callers_caller: _,
        modifier,
    } = request;

    debug_assert_eq!(callee, L1_MESSENGER_ADDRESS_HOOK);

    // Can be used only by L1 messenger system contract
    if caller != L1_MESSENGER_ADDRESS {
        system_log!(
            system,
            "L1 messenger hook: invalid caller (caller={caller:?})\n"
        );
        // Pretend to be an empty account
        return Ok((
            make_return_state_from_returndata_region(available_resources, &[]),
            return_memory,
        ));
    }

    // Note: it's ok to revert below even when it breaks EVM compatibility, since
    // the L1 messenger system contract should guarantee correct usage.

    let mut error = false;
    // This hook doesn't accept any native token value
    error |= nominal_token_value != U256::ZERO;
    let mut is_static = false;
    match modifier {
        CallModifier::Constructor => {
            return Err(
                internal_error!("L1 messenger hook called with constructor modifier").into(),
            )
        }
        CallModifier::Delegate
        | CallModifier::DelegateStatic
        | CallModifier::EVMCallcode
        | CallModifier::EVMCallcodeStatic => {
            error = true;
        }
        CallModifier::Static | CallModifier::ZKVMSystemStatic => {
            is_static = true;
        }
        _ => {}
    }

    if error {
        return Ok((make_error_return_state(available_resources), return_memory));
    }

    let mut resources = available_resources;

    let result = l1_messenger_hook_inner(&calldata, &mut resources, system, caller_ee, is_static);

    match result {
        Ok(Ok(())) => Ok((
            make_return_state_from_returndata_region(resources, &[]),
            return_memory,
        )),
        Ok(Err(e)) => {
            system_log!(system, "Revert: {e:?}\n");
            Ok((make_error_return_state(resources), return_memory))
        }
        Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
            system_log!(system, "Out of gas during system hook\n");
            Ok((make_error_return_state(resources), return_memory))
        }
        Err(e @ SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(_))) => Err(e),
        Err(SystemError::LeafDefect(e)) => Err(e.into()),
    }
}

fn l1_messenger_hook_inner<S: EthereumLikeTypes>(
    calldata: &[u8],
    resources: &mut S::Resources,
    system: &mut System<S>,
    _caller_ee: u8,
    is_static: bool,
) -> Result<Result<(), &'static str>, SystemError>
where
{
    evm_interpreter::charge_native_and_ergs::<S::Resources>(
        resources,
        HOOK_BASE_NATIVE_COST,
        Ergs(0), // Do not charge EVM gas here, it is already charged in L1Messenger smart contract
    )?;

    // Should never happen
    if is_static {
        return Ok(Err(
            "L1 messenger failure: sendToL1 called with static context",
        ));
    }

    send_to_l1_inner(&calldata, resources, system)
}

/// Receives calldata in the form of abi.encodePacked(address msg.sender, bytes message)
/// Only sends a message to L1 (emit_l1_message), events are emitted on the contract level.
/// Returns nothing.
pub(crate) fn send_to_l1_inner<S: EthereumLikeTypes>(
    calldata: &[u8],
    resources: &mut S::Resources,
    system: &mut System<S>,
) -> Result<Result<(), &'static str>, SystemError> {
    if calldata.len() < 20 {
        return Ok(Err(
            "L1 messenger failure: sendToL1 called with invalid calldata",
        ));
    }

    let address_sender = B160::try_from_be_slice(&calldata[0..20]).ok_or(
        SystemError::LeafDefect(internal_error!("Failed to create B160 from 20 byte array")),
    )?;

    let message = &calldata[20..];

    // emit L1 message (ignore returned hash)
    // TODO(EVM-1190): hash calculation is suboptimal, to be refactored in future
    system.io.emit_l1_message(
        // Gas should be charged by the L1Messenger system contract
        ExecutionEnvironmentType::NoEE,
        resources,
        &address_sender,
        message,
    )?;

    Ok(Ok(()))
}

//!
//! L1 messenger system hook implementation.
//! It implements a `sendToL1` method, works the same way as in Era.
//!
use super::*;
use crate::addresses_constants::{L1_MESSENGER_ADDRESS, L1_MESSENGER_ADDRESS_HOOK};
use core::fmt::Write;
use evm_interpreter::{
    gas_constants::{LOG, LOGDATA},
    keccak256_ergs_cost,
};
use ruint::aliases::{B160, U256};
use zk_ee::{
    common_structs::L2_TO_L1_LOG_SERIALIZE_SIZE,
    execution_environment_type::ExecutionEnvironmentType,
    internal_error, out_of_return_memory,
    system::{
        errors::{runtime::RuntimeError, system::SystemError},
        CallModifier, CompletedExecution, ExternalCallRequest,
    },
    utils::Bytes32,
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

    if caller != L1_MESSENGER_ADDRESS || callee != L1_MESSENGER_ADDRESS_HOOK {
        let _ = system.get_logger().write_fmt(format_args!(
            "Set bytecode hook revert: invalid caller (caller={caller:?}, callee={callee:?})\n"
        ));
        return Ok((make_error_return_state(available_resources), return_memory));
    }

    let mut error = false;
    // There are no "payable" methods
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
        Ok(Ok(return_data)) => {
            let mut return_memory = SliceVec::new(return_memory);
            // TODO: check endianness
            return_memory
                .try_extend(return_data.as_u8_ref().iter().copied())
                .map_err(|_| out_of_return_memory!())?;
            let (returndata, rest) = return_memory.destruct();
            Ok((
                make_return_state_from_returndata_region(resources, returndata),
                rest,
            ))
        }
        Ok(Err(e)) => {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Revert: {e:?}\n"));
            Ok((make_error_return_state(resources), return_memory))
        }
        Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Out of gas during system hook\n"));
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
) -> Result<Result<Bytes32, &'static str>, SystemError>
where
{
    evm_interpreter::charge_native_and_ergs::<S::Resources>(
        resources,
        HOOK_BASE_NATIVE_COST,
        HOOK_BASE_ERGS_COST,
    )?;

    if is_static {
        return Ok(Err(
            "L1 messenger failure: sendToL1 called with static context",
        ));
    }

    send_to_l1_inner(&calldata, resources, system)
}

/// Only sends a message to L1 (emit_l1_message), events are emitted on the contract level.
/// Returns message hash.
pub(crate) fn send_to_l1_inner<S: EthereumLikeTypes>(
    calldata: &[u8],
    resources: &mut S::Resources,
    system: &mut System<S>,
) -> Result<Result<Bytes32, &'static str>, SystemError> {
    let address_sender = B160::try_from_be_slice(&calldata[12..32]).ok_or(
        SystemError::LeafDefect(internal_error!("Failed to create B160 from 20 byte array")),
    )?;

    let offset: usize = U256::from_be_slice(&calldata[32..64])
        .try_into()
        .map_err(|_| SystemError::LeafDefect(internal_error!("Invalid offset word")))?;

    let length: usize = U256::from_be_slice(&calldata[offset..offset + 32])
        .try_into()
        .map_err(|_| SystemError::LeafDefect(internal_error!("Invalid length word")))?;

    let start = offset + 32;
    let end = start + length;
    if calldata.len() < end {
        return Ok(Err("L1 messenger failure: truncated bytes payload"));
    }
    let message = &calldata[start..end];

    // charge for message length
    let l1_message_cost = l1_message_ergs_cost(message.len());
    resources.charge(&S::Resources::from_ergs(l1_message_cost))?;

    // emit L1 message
    let message_hash = system.io.emit_l1_message(
        ExecutionEnvironmentType::NoEE,
        resources,
        &address_sender,
        message,
    )?;

    Ok(Ok(message_hash))
}

///
/// Ergs cost of emitting an L1 message.
/// Computed as:
///   keccak256_ergs_cost(L2_TO_L1_LOG_SERIALIZE_SIZE) +
///   keccak256_ergs_cost(64) * 3 +
///   keccak256_ergs_cost(message_len) +
///   375 (same as LOG base) +
///   8 * message_len (same as LOG for data)
///
/// See [io_subsystem::emit_l1_message] for more details
/// about the 3 first components of this calculation.
///
fn l1_message_ergs_cost(message_len: usize) -> Ergs {
    let hashing_cost = keccak256_ergs_cost(L2_TO_L1_LOG_SERIALIZE_SIZE)
        + keccak256_ergs_cost(64).times(3)
        + keccak256_ergs_cost(message_len);
    let log_cost = Ergs(ERGS_PER_GAS * (LOG + LOGDATA * message_len as u64));
    hashing_cost + log_cost
}

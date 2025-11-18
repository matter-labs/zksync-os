//!
//! L2 base token system hook implementation.
//!
//! This module provides the withdrawal functionality for the L2 base token (ETH equivalent).
//! It implements methods for `withdraw` and `withdrawWithMessage`, which work in the same way as in Era.
//!
//! ## Supported Operations
//! - `withdraw(address)` - Burns L2 tokens and initiates withdrawal to L1 receiver
//! - `withdrawWithMessage(address,bytes)` - Burns L2 tokens with additional data for L1 processing
//!
//! ## Notes
//! - Minting is performed in the bootloader automatically with corresponding "Mint" events if L1->L2 or upgrade tx has some value attached

use super::*;
use core::fmt::Write;
use ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::errors::{runtime::RuntimeError, system::SystemError};
use zk_ee::utils::b160_to_u256;
use zk_ee::{internal_error, out_of_return_memory};

pub fn burn_native_token_hook<'a, S: EthereumLikeTypes>(
    request: ExternalCallRequest<S>,
    caller_ee: u8,
    system: &mut System<S>,
    return_memory: &'a mut [MaybeUninit<u8>],
) -> Result<(CompletedExecution<'a, S>, &'a mut [MaybeUninit<u8>]), SystemError>
where
    S::IO: IOSubsystemExt,
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

    debug_assert_eq!(callee, BURN_NATIVE_TOKEN_ADDRESS);

    let mut error = false;
    let mut is_static = false;
    match modifier {
        CallModifier::Constructor => {
            return Err(
                internal_error!("L2 base token hook called with constructor modifier").into(),
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

    let result = burn_native_token_hook_inner(
        &calldata,
        &mut resources,
        system,
        caller,
        caller_ee,
        nominal_token_value,
        is_static,
    );

    match result {
        Ok(Ok(return_data)) => {
            let mut return_memory = SliceVec::new(return_memory);
            return_memory
                .try_extend(return_data.iter().copied())
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

fn burn_native_token_hook_inner<S: EthereumLikeTypes>(
    calldata: &[u8],
    resources: &mut S::Resources,
    system: &mut System<S>,
    caller: B160,
    _caller_ee: u8,
    nominal_token_value: U256,
    is_static: bool,
) -> Result<Result<&'static [u8], &'static str>, SystemError>
where
    S::IO: IOSubsystemExt,
{
    let _ = system.get_logger().write_fmt(format_args!(
        "burn hook: caller=0x{:x} callee=0x{:x} nominal={}\n",
        b160_to_u256(caller),
        b160_to_u256(BURN_NATIVE_TOKEN_ADDRESS),
        nominal_token_value
    ));

    evm_interpreter::charge_native_and_ergs::<S::Resources>(
        resources,
        HOOK_BASE_NATIVE_COST,
        HOOK_BASE_ERGS_COST,
    )?;

    // 1) This operation must not run in STATICCALL
    if is_static {
        return Ok(Err("L2 base token: burn called with static context"));
    }

    // 2) We accept *no* arguments; optionally enforce empty calldata
    if !calldata.is_empty() {
        // If you prefer to silently ignore calldata, remove this check.
        return Ok(Err("L2 base token: burn takes no arguments"));
    }

    // 3) Amount to burn is whatever was sent with the call (aka msg.value)
    if nominal_token_value.is_zero() {
        // Up to you: treat zero-value as no-op success or revert.
        // Reverting gives cleaner semantics:
        return Ok(Err("L2 base token: nothing to burn (zero value)"));
    }

    let _ = system.get_logger().write_fmt(format_args!(
        "burning from=0x{:x} amount={}\n",
        b160_to_u256(L2_BASE_TOKEN_ADDRESS), // or BURN_NATIVE_TOKEN_ADDRESS depending on pattern
        nominal_token_value
    ));

    // 4) Burn the nominal value (same burn primitive you already use)
    burn_nominal_token_value(resources, system, &nominal_token_value)?;

    Ok(Ok(&[]))
}

/// Burns the specified amount of nominal tokens from the L2 base token contract
fn burn_nominal_token_value<S: EthereumLikeTypes>(
    resources: &mut S::Resources,
    system: &mut System<S>,
    nominal_token_value: &U256,
) -> Result<(), SystemError>
where
    S::IO: IOSubsystemExt,
{
    match system.io.update_account_nominal_token_balance(
        // Use EVM EE to charge for gas too
        ExecutionEnvironmentType::EVM,
        resources,
        &BURN_NATIVE_TOKEN_ADDRESS,
        &nominal_token_value,
        true,
    ) {
        Ok(_) => Ok(()),
        // TODO this has to be properly propagated
        Err(SubsystemError::LeafUsage(_)) => Err(SystemError::LeafDefect(internal_error!(
            "L2 base token must have withdrawal amount"
        ))),
        Err(SubsystemError::LeafRuntime(e)) => Err(e.into()),
        Err(SubsystemError::LeafDefect(e)) => Err(e.into()),
        Err(SubsystemError::Cascaded(e)) => match e {},
    }
}

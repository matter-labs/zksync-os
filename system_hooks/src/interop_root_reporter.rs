//!
//! Interop root reporter system hook implementation.
//!
use super::*;
use core::fmt::Write;
use ruint::aliases::U256;
use zk_ee::{
    common_structs::interop_root_storage::InteropRoot,
    internal_error,
    system::{
        errors::{runtime::RuntimeError, system::SystemError},
        CallModifier, CompletedExecution, ExternalCallRequest,
    },
    utils::Bytes32,
};

pub fn interop_root_reporter_hook<'a, S: EthereumLikeTypes>(
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

    debug_assert_eq!(caller, L2_INTEROP_ROOT_STORAGE_ADDRESS);
    debug_assert_eq!(callee, INTEROP_ROOT_REPORTER_ADDRESS_HOOK);

    let mut error = false;
    // There are no "payable" methods
    error |= nominal_token_value != U256::ZERO;
    let mut is_static = false;
    match modifier {
        CallModifier::Constructor => {
            return Err(internal_error!(
                "Interop root reporter hook called with constructor modifier"
            )
            .into())
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

    let result =
        interop_root_reporter_inner(&calldata, &mut resources, system, caller_ee, is_static);

    match result {
        Ok(Ok(())) => {
            let return_memory = SliceVec::new(return_memory);
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

fn interop_root_reporter_inner<S: EthereumLikeTypes>(
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
        HOOK_BASE_ERGS_COST,
    )?;

    if calldata.len() != 96 {
        return Ok(Err(
            "Interop root reporter failure: calldata length mismatch",
        ));
    }

    if is_static {
        return Ok(Err(
            "Interop root reporter failure: called with static context",
        ));
    }

    report_inner(&calldata, resources, system)
}

/// Saves an interop root passed as calldata following the encoding:
/// [ 0..31] chainId
/// [32..63] blockOrBatchNumber
/// [64..95] root
pub(crate) fn report_inner<S: EthereumLikeTypes>(
    calldata: &[u8],
    _resources: &mut S::Resources,
    system: &mut System<S>,
) -> Result<Result<(), &'static str>, SystemError> {
    let chain_id = U256::from_be_slice(&calldata[0..32]);
    let block_or_batch_number = U256::from_be_slice(&calldata[32..64]);
    let root = Bytes32::from_array(calldata[64..96].try_into().unwrap());

    // TODO: charge ergs for storing the root
    system.io.add_interop_root(InteropRoot {
        root,
        block_or_batch_number,
        chain_id,
    })?;

    Ok(Ok(()))
}

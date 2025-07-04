//!
//! This module contains EVM precompiles system hooks implementations.
//! For most of them there are corresponding functions in the system.
//! So here is a generic wrapper to wrap the system function into the system hook.
//! Currently supported precompiles:
//! - ecrecover
//! - sha256
//! - ripemd-160
//! - identity (there is no system function, see `id_hook` below for more details)
//! - modexp
//! - ecadd
//! - ecmul
//! - ecpairing
//!
use super::*;
use core::fmt::Write;
use evm_interpreter::ERGS_PER_GAS;
use zk_ee::system::{
    errors::{SystemError, SystemFunctionError},
    CallModifier, Resources, System,
};

///
/// Generic system function hook implementation.
/// It parses call request, calls system function, and creates execution result.
///
/// NOTE: "pure" here means that we do not expect to trigger any state changes (and calling with static flag is ok),
/// so for all the purposes we remain in the callee frame in terms of memory for efficiency
///
pub fn pure_system_function_hook_impl<'a, F: SystemFunction<S::Resources>, S: EthereumLikeTypes>(
    request: ExternalCallRequest<S>,
    _caller_ee: u8,
    system: &mut System<S>,
    return_memory: &'a mut [MaybeUninit<u8>],
) -> Result<(CompletedExecution<'a, S>, &'a mut [MaybeUninit<u8>]), FatalError> {
    let ExternalCallRequest {
        available_resources,
        calldata,
        modifier,
        ..
    } = request;

    // We allow static calls as we are "pure" hook
    if modifier == CallModifier::Constructor {
        return Err(InternalError("precompile called with constructor modifier").into());
    }

    let mut resources = available_resources;

    let allocator = system.get_allocator();

    let mut return_vec = SliceVec::new(return_memory);
    let result = F::execute(&calldata, &mut return_vec, &mut resources, allocator);

    match result {
        Ok(()) => {
            let (returndata, rest) = return_vec.destruct();
            Ok((
                make_return_state_from_returndata_region(resources, returndata),
                rest,
            ))
        }
        Err(SystemFunctionError::System(SystemError::OutOfErgs))
        | Err(SystemFunctionError::InvalidInput) => {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Out of gas during system hook\n"));
            resources.exhaust_ergs();
            let (_, rest) = return_vec.destruct();
            Ok((make_error_return_state(resources), rest))
        }
        Err(SystemFunctionError::System(SystemError::OutOfNativeResources)) => {
            Err(FatalError::OutOfNativeResources)
        }
        Err(SystemFunctionError::System(SystemError::Internal(e))) => Err(e.into()),
    }
}

/// as there is no system function for identity(memcopy)
/// we define one following the system functions interface
/// to use same logic as for other hooks
pub struct IdentityPrecompile;
const ID_STATIC_COST_ERGS: Ergs = Ergs(15 * ERGS_PER_GAS);
const ID_WORD_COST_ERGS: Ergs = Ergs(3 * ERGS_PER_GAS);
const ID_BASE_NATIVE_COST: u64 = 20;
const ID_BYTE_NATIVE_COST: u64 = 10;
impl<R: Resources> SystemFunction<R> for IdentityPrecompile {
    fn execute<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        src: &[u8],
        dst: &mut D,
        resources: &mut R,
        _: A,
    ) -> Result<(), SystemFunctionError> {
        cycle_marker::wrap_with_resources!("id", resources, {
            let cost_ergs =
                ID_STATIC_COST_ERGS + ID_WORD_COST_ERGS.times((src.len() as u64).div_ceil(32));
            let cost_native = ID_BASE_NATIVE_COST + ID_BYTE_NATIVE_COST * (src.len() as u64);
            resources.charge(&R::from_ergs_and_native(
                cost_ergs,
                <R::Native as zk_ee::system::Computational>::from_computational(cost_native),
            ))?;
            dst.extend(src.iter().cloned());
            Ok(())
        })
    }
}

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

struct QuasiVec<'a, S: SystemTypes> {
    buffer: OSResizableSlice<S>,
    offset: usize,
    system: &'a mut System<S>,
}

impl<'a, S: SystemTypes> QuasiVec<'a, S> {
    const INITIAL_LEN: usize = 32;

    fn new(system: &'a mut System<S>) -> Self {
        let buffer = system.memory.empty_managed_region();
        let buffer = system
            .memory
            .grow_heap(buffer, Self::INITIAL_LEN)
            .expect("must grow buffer for precompiles")
            .expect("must grow buffer for precompiles");

        Self {
            buffer,
            offset: 0,
            system,
        }
    }
}

impl<'a, S: SystemTypes> Extend<u8> for QuasiVec<'a, S> {
    fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
        for byte in iter {
            if self.offset == self.buffer.len() {
                // grow
                let new_len = self.buffer.len() * 2;
                let buffer =
                    core::mem::replace(&mut self.buffer, self.system.memory.empty_managed_region());
                self.buffer = self
                    .system
                    .memory
                    .grow_heap(buffer, new_len)
                    .expect("must grow buffer for precompiles")
                    .expect("must grow buffer for precompiles");
            }
            unsafe {
                core::hint::assert_unchecked(self.buffer.len() >= self.offset);
            }
            self.buffer[self.offset] = byte;
            self.offset += 1;
        }
    }
}

///
/// Generic system function hook implementation.
/// It parses call request, calls system function, and creates execution result.
///
/// NOTE: "pure" here means that we do not expect to trigger any state changes (and calling with static flag is ok),
/// so for all the purposes we remain in the callee frame in terms of memory for efficiency
///
pub fn pure_system_function_hook_impl<F: SystemFunction<S::Resources>, S: EthereumLikeTypes>(
    request: ExternalCallRequest<S>,
    _caller_ee: u8,
    system: &mut System<S>,
) -> Result<CompletedExecution<S>, FatalError>
where
    S::Memory: MemorySubsystemExt,
{
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
    // NOTE: we did NOT start a frame here, so we are in the caller frame in terms of memory, and must be extra careful
    // here on how we will make returndata

    let mut resources = available_resources;

    let allocator = system.get_allocator();
    // TODO: use returndata region directly

    // cheat
    let snapshot = system.memory.start_memory_frame();
    let mut buffer = QuasiVec::new(system);
    let result = F::execute(&calldata, &mut buffer, &mut resources, allocator);
    let returndata = if result.is_ok() {
        let QuasiVec {
            buffer,
            offset,
            system,
        } = buffer;
        // copy it
        system
            .memory
            .copy_into_return_memory(&buffer[..offset])
            .expect("must copy into returndata")
            .take_slice(0..offset)
    } else {
        system.memory.empty_immutable_slice()
    };
    system.memory.finish_memory_frame(Some(snapshot));

    match result {
        Ok(()) => Ok(make_return_state_from_returndata_region(
            system, resources, returndata,
        )),
        Err(SystemFunctionError::System(SystemError::OutOfErgs))
        | Err(SystemFunctionError::InvalidInput) => {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Out of gas during system hook\n"));
            resources.exhaust_ergs();
            Ok(make_error_return_state(system, resources))
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

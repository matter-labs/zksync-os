use super::*;

use crate::cost_constants::{
    RIPEMD160_BASE_NATIVE_COST, RIPEMD160_CHUNK_SIZE, RIPEMD160_ROUND_NATIVE_COST,
    RIPEMD_160_PER_WORD_COST_ERGS, RIPEMD_160_STATIC_COST_ERGS,
};
use zk_ee::system::base_system_functions::{RipeMd160Errors, SystemFunction};
use zk_ee::system::errors::{subsystem::SubsystemError, system::SystemError};
use zk_ee::system::Computational;

///
/// ripemd-160 system function implementation.
///
pub struct RipeMd160Impl;
impl<R: Resources> SystemFunction<R, RipeMd160Errors> for RipeMd160Impl {
    /// Returns `OutOfGas` if not enough resources provided.
    fn execute<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        _allocator: A,
    ) -> Result<(), SubsystemError<RipeMd160Errors>> {
        Ok(cycle_marker::wrap_with_resources!("ripemd", resources, {
            ripemd160_as_system_function_inner(input, output, resources)
        })?)
    }
}

fn nb_rounds(len: usize) -> u64 {
    let full_chunks = len / RIPEMD160_CHUNK_SIZE;
    let tail = len % RIPEMD160_CHUNK_SIZE;
    let num_rounds: u64 = full_chunks as u64;
    if tail <= 55 {
        num_rounds + 1
    } else {
        num_rounds + 2
    }
}

fn ripemd160_as_system_function_inner<D: ?Sized + Extend<u8>, R: Resources>(
    input: &[u8],
    dst: &mut D,
    resources: &mut R,
) -> Result<(), SystemError> {
    let word_size = (input.len() as u64).div_ceil(32);
    let ergs_cost = RIPEMD_160_STATIC_COST_ERGS + RIPEMD_160_PER_WORD_COST_ERGS.times(word_size);
    let native_cost =
        RIPEMD160_BASE_NATIVE_COST + nb_rounds(input.len()) * RIPEMD160_ROUND_NATIVE_COST;
    resources.charge(&R::from_ergs_and_native(
        ergs_cost,
        <R::Native as Computational>::from_computational(native_cost),
    ))?;

    use crypto::ripemd160::*;
    let mut hasher = Ripemd160::new();
    hasher.update(input);
    let hash = hasher.finalize();

    dst.extend(core::iter::repeat_n(0, 12).chain(hash));

    Ok(())
}

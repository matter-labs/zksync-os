use super::*;
use crate::cost_constants::{BN254_ECADD_COST_GAS, BN254_ECADD_NATIVE_COST};
use crypto::precompiles::bn254_g1_add;
use zk_ee::common_traits::TryExtend;
use zk_ee::oracle::IOOracle;
use zk_ee::system::base_system_functions::{
    Bn254AddErrors, Bn254AddInterfaceError, SystemFunctionExt,
};
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::logger::Logger;
use zk_ee::{interface_error, out_of_return_memory};

///
/// bn254 ecadd system function implementation.
/// With `USE_ADVICE`, the inversion that makes the result affine is taken from an oracle hint.
///
pub struct Bn254AddImpl<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, Bn254AddErrors>
    for Bn254AddImpl<USE_ADVICE>
{
    /// Returns the size in bytes of output.
    ///
    /// If the input size is less than expected - it will be padded with zeroes.
    /// If the input size is greater - redundant bytes will be ignored.
    ///
    /// If output len less than needed(64) returns `InternalError`.
    /// Returns `OutOfGas` if not enough resources provided.
    /// Returns `InvalidInput` error only if failed to create affine points from inputs.
    fn execute<
        O: IOOracle,
        L: Logger,
        D: TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        src: &[u8],
        dst: &mut D,
        resources: &mut R,
        oracle: &mut O,
        _logger: &mut L,
        _: A,
    ) -> Result<(), SubsystemError<Bn254AddErrors>> {
        cycle_marker::wrap_with_resources!("bn254_ecadd", resources, {
            let oracle = if USE_ADVICE { Some(oracle) } else { None };
            bn254_ecadd_as_system_function_inner(src, dst, resources, oracle)
        })
    }
}

fn bn254_ecadd_as_system_function_inner<
    S: ?Sized + MinimalByteAddressableSlice,
    D: ?Sized + TryExtend<u8>,
    R: Resources,
    O: crypto::hints::MemoryOracle,
>(
    src: &S,
    dst: &mut D,
    resources: &mut R,
    oracle: Option<&mut O>,
) -> Result<(), SubsystemError<Bn254AddErrors>> {
    resources.charge_legacy_gas_and_native(BN254_ECADD_COST_GAS, BN254_ECADD_NATIVE_COST)?;

    let mut buffer = [0u8; 128];
    for (dst, src) in buffer.iter_mut().zip(src.iter()) {
        *dst = *src;
    }

    let coordinates = buffer.as_chunks::<64>().0.try_into().unwrap();

    let serialized_result =
        bn254_g1_add(coordinates, oracle).map_err(|_| -> SubsystemError<_> {
            interface_error!(Bn254AddInterfaceError::InvalidPoint)
        })?;

    dst.try_extend_from_slice(&serialized_result)
        .map_err(|_| out_of_return_memory!())?;

    Ok(())
}

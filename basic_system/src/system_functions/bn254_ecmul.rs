use super::*;
use crate::cost_constants::BN254_ECMUL_NATIVE_COST;
use crate::{
    cost_constants::BN254_ECMUL_COST_GAS,
    system_functions::bn254_ecadd::{bigint_from_be, parse_affine, serialize_projective},
};
use zk_ee::common_traits::TryExtend;
use zk_ee::oracle::IOOracle;
use zk_ee::system::base_system_functions::{
    Bn254MulErrors, Bn254MulInterfaceError, SystemFunctionExt,
};
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::logger::Logger;
use zk_ee::{interface_error, out_of_return_memory};

///
/// bn254 ecmul system function implementation.
/// With `USE_ADVICE`, the inversion that makes the result affine is taken from an oracle hint.
///
pub struct Bn254MulImpl<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, Bn254MulErrors>
    for Bn254MulImpl<USE_ADVICE>
{
    /// If the input size is less than expected - it will be padded with zeroes.
    /// If the input size is greater - redundant bytes will be ignored.
    ///
    /// Returns `OutOfGas` if not enough resources provided.
    /// Returns `InvalidInput` error only if failed to create affine points from inputs.
    fn execute<
        O: IOOracle,
        L: Logger,
        D: TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        oracle: &mut O,
        _logger: &mut L,
        _allocator: A,
    ) -> Result<(), SubsystemError<Bn254MulErrors>> {
        cycle_marker::wrap_with_resources!("bn254_ecmul", resources, {
            let oracle = if USE_ADVICE { Some(oracle) } else { None };
            bn254_ecmul_as_system_function_inner(input, output, resources, oracle)
        })
    }
}

fn bn254_ecmul_as_system_function_inner<
    S: ?Sized + MinimalByteAddressableSlice,
    D: ?Sized + TryExtend<u8>,
    R: Resources,
    O: IOOracle,
>(
    src: &S,
    dst: &mut D,
    resources: &mut R,
    oracle: Option<&mut O>,
) -> Result<(), SubsystemError<Bn254MulErrors>> {
    resources.charge_legacy_gas_and_native(BN254_ECMUL_COST_GAS, BN254_ECMUL_NATIVE_COST)?;

    let mut buffer = [0u8; 96];
    for (dst, src) in buffer.iter_mut().zip(src.iter()) {
        *dst = *src;
    }

    let mut it = buffer.as_chunks::<32>().0.iter();
    let serialized_result = unsafe {
        let x0 = it.next().unwrap_unchecked();
        let y0 = it.next().unwrap_unchecked();
        let scalar = it.next().unwrap_unchecked();

        bn254_ecmul_inner(x0, y0, scalar, oracle).map_err(|_| -> SubsystemError<_> {
            interface_error!(Bn254MulInterfaceError::InvalidPoint)
        })?
    };

    dst.try_extend_from_slice(&serialized_result)
        .map_err(|_| out_of_return_memory!())?;

    Ok(())
}

/// With an oracle, the inversion that makes the result affine comes from a checked hint.
pub fn bn254_ecmul_inner<O: IOOracle>(
    x: &[u8; 32],
    y: &[u8; 32],
    scalar: &[u8; 32],
    oracle: Option<&mut O>,
) -> Result<[u8; 64], ()> {
    use crypto::ark_ec::AffineRepr;

    let is_zero = x.iter().all(|el| *el == 0) && y.iter().all(|el| *el == 0);
    if is_zero {
        return Ok([0u8; 64]);
    }
    let affine_point = parse_affine(x, y)?;
    // the scalar is not reduced: any 256-bit integer is a valid multiplier
    let scalar = bigint_from_be(scalar);

    let result = affine_point.mul_bigint(&scalar);
    let result = serialize_projective(result, oracle);

    Ok(result)
}

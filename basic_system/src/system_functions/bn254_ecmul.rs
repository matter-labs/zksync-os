use super::*;
use crate::cost_constants::BN254_ECMUL_NATIVE_COST;
use crate::system_functions::bytereverse;
use crate::{
    cost_constants::BN254_ECMUL_COST_ERGS, system_functions::bn254_ecadd::serialize_projective,
};
use crypto::ark_serialize::Valid;
use zk_ee::common_traits::TryExtend;
use zk_ee::system::base_system_functions::{
    Bn254MulErrors, Bn254MulInterfaceError, SystemFunction,
};
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::{interface_error, out_of_return_memory};

///
/// bn254 ecmul system function implementation.
///
pub struct Bn254MulImpl;

impl<R: Resources> SystemFunction<R, Bn254MulErrors> for Bn254MulImpl {
    /// If the input size is less than expected - it will be padded with zeroes.
    /// If the input size is greater - redundant bytes will be ignored.
    ///
    /// Returns `OutOfGas` if not enough resources provided.
    /// Returns `InvalidInput` error only if failed to create affine points from inputs.
    fn execute<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        _allocator: A,
    ) -> Result<(), SubsystemError<Bn254MulErrors>> {
        cycle_marker::wrap_with_resources!("bn254_ecmul", resources, {
            bn254_ecmul_as_system_function_inner(input, output, resources)
        })
    }
}

fn bn254_ecmul_as_system_function_inner<
    S: ?Sized + MinimalByteAddressableSlice,
    D: ?Sized + TryExtend<u8>,
    R: Resources,
>(
    src: &S,
    dst: &mut D,
    resources: &mut R,
) -> Result<(), SubsystemError<Bn254MulErrors>> {
    resources.charge(&R::from_ergs_and_native(
        BN254_ECMUL_COST_ERGS,
        <R::Native as zk_ee::system::Computational>::from_computational(BN254_ECMUL_NATIVE_COST),
    ))?;

    let mut buffer = [0u8; 96];
    for (dst, src) in buffer.iter_mut().zip(src.iter()) {
        *dst = *src;
    }

    let mut it = buffer.as_chunks::<32>().0.iter();
    let serialized_result = unsafe {
        let x0 = it.next().unwrap_unchecked();
        let y0 = it.next().unwrap_unchecked();
        let scalar = it.next().unwrap_unchecked();

        bn254_ecmul_inner(x0, y0, scalar).map_err(|_| -> SubsystemError<_> {
            interface_error!(Bn254MulInterfaceError::InvalidPoint)
        })?
    };

    dst.try_extend(serialized_result)
        .map_err(|_| out_of_return_memory!())?;

    Ok(())
}

pub fn bn254_ecmul_inner(x: &[u8; 32], y: &[u8; 32], scalar: &[u8; 32]) -> Result<[u8; 64], ()> {
    use crypto::ark_ec::AffineRepr;
    use crypto::ark_ff::PrimeField;
    use crypto::ark_serialize::CanonicalDeserialize;
    use crypto::bn254::*;

    let is_zero = x.iter().all(|el| *el == 0) && y.iter().all(|el| *el == 0);
    if is_zero {
        return Ok([0u8; 64]);
    }
    let mut x = *x;
    let mut y = *y;
    bytereverse(&mut x);
    bytereverse(&mut y);
    let x_bigint = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&x[..]).map_err(|_| ())?;
    let y_bigint = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&y[..]).map_err(|_| ())?;
    let x_coordinate = Fq::from_bigint(x_bigint).ok_or(())?;
    let y_coordinate = Fq::from_bigint(y_bigint).ok_or(())?;
    let affine_point = G1Affine::new_unchecked(x_coordinate, y_coordinate);
    affine_point.check().map_err(|_| ())?;

    let mut scalar = *scalar;
    bytereverse(&mut scalar);
    let scalar =
        <Fr as PrimeField>::BigInt::deserialize_uncompressed(&scalar[..]).map_err(|_| ())?;

    let result = affine_point.mul_bigint(&scalar);
    let result = serialize_projective(result);

    Ok(result)
}

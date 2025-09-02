use super::*;
use crate::cost_constants::{BN254_ECADD_COST_ERGS, BN254_ECADD_NATIVE_COST};
use crate::system_functions::bytereverse;
use crypto::ark_ec::CurveGroup;
use crypto::ark_ff::PrimeField;
use crypto::ark_serialize::{CanonicalSerialize, Valid};
use zk_ee::interface_error;
use zk_ee::system::base_system_functions::{
    Bn254AddErrors, Bn254AddInterfaceError, SystemFunction,
};
use zk_ee::system::errors::subsystem::SubsystemError;

///
/// bn254 ecadd system function implementation.
///
pub struct Bn254AddImpl;

impl<R: Resources> SystemFunction<R, Bn254AddErrors> for Bn254AddImpl {
    /// Returns the size in bytes of output.
    ///
    /// If the input size is less than expected - it will be padded with zeroes.
    /// If the input size is greater - redundant bytes will be ignored.
    ///
    /// If output len less than needed(64) returns `InternalError`.
    /// Returns `OutOfGas` if not enough resources provided.
    /// Returns `InvalidInput` error only if failed to create affine points from inputs.
    fn execute<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        src: &[u8],
        dst: &mut D,
        resources: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<Bn254AddErrors>> {
        cycle_marker::wrap_with_resources!("bn254_ecadd", resources, {
            bn254_ecadd_as_system_function_inner(src, dst, resources)
        })
    }
}

fn bn254_ecadd_as_system_function_inner<
    S: ?Sized + MinimalByteAddressableSlice,
    D: ?Sized + Extend<u8>,
    R: Resources,
>(
    src: &S,
    dst: &mut D,
    resources: &mut R,
) -> Result<(), SubsystemError<Bn254AddErrors>> {
    resources.charge(&R::from_ergs_and_native(
        BN254_ECADD_COST_ERGS,
        <R::Native as zk_ee::system::Computational>::from_computational(BN254_ECADD_NATIVE_COST),
    ))?;

    let mut buffer = [0u8; 128];
    for (dst, src) in buffer.iter_mut().zip(src.iter()) {
        *dst = *src;
    }

    let coordinates = buffer.as_chunks::<64>().0.try_into().unwrap();

    let serialized_result = bn254_ecadd_inner(coordinates).map_err(|_| -> SubsystemError<_> {
        interface_error!(Bn254AddInterfaceError::InvalidPoint)
    })?;

    dst.extend(serialized_result);

    Ok(())
}

pub fn bn254_ecadd_inner(coordinates: &[[u8; 64]; 2]) -> Result<[u8; 64], ()> {
    use crypto::ark_ec::AffineRepr;
    use crypto::ark_ff::PrimeField;
    use crypto::ark_serialize::CanonicalDeserialize;
    use crypto::bn254::*;

    let mut points = [G1Affine::identity(); 2];
    for (dst, xy) in points.iter_mut().zip(coordinates.iter()) {
        let [mut x, mut y] = xy.as_chunks::<32>().0.try_into().unwrap();

        let is_zero = x.iter().all(|el| *el == 0) && y.iter().all(|el| *el == 0);
        if is_zero {
            continue;
        }

        bytereverse(&mut x);
        bytereverse(&mut y);
        let x_bigint =
            <Fq as PrimeField>::BigInt::deserialize_uncompressed(&x[..]).map_err(|_| ())?;
        let y_bigint =
            <Fq as PrimeField>::BigInt::deserialize_uncompressed(&y[..]).map_err(|_| ())?;
        let x_coordinate = Fq::from_bigint(x_bigint).ok_or(())?;
        let y_coordinate = Fq::from_bigint(y_bigint).ok_or(())?;
        let affine_point = G1Affine::new_unchecked(x_coordinate, y_coordinate);
        affine_point.check().map_err(|_| ())?;
        *dst = affine_point;
    }

    let [a, b] = points;
    let mut result: G1Projective = a.into_group();
    result += &b;
    let result = serialize_projective(result);

    Ok(result)
}

pub(crate) fn serialize_projective(point: crypto::bn254::G1Projective) -> [u8; 64] {
    use crypto::ark_ec::AffineRepr;
    use crypto::ark_ff::Zero;
    if point.is_zero() {
        // canonical for zero point
        [0u8; 64]
    } else {
        let result = point.into_affine();
        let (x, y) = result.xy().unwrap();
        let x_bigint = x.into_bigint();
        let y_bigint = y.into_bigint();
        let mut result = [0u8; 64];
        x_bigint.serialize_uncompressed(&mut result[0..32]).unwrap();
        bytereverse(&mut result[0..32]);
        y_bigint
            .serialize_uncompressed(&mut result[32..64])
            .unwrap();
        bytereverse(&mut result[32..64]);

        result
    }
}

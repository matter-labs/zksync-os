use super::*;
use crate::cost_constants::{BN254_ECADD_COST_GAS, BN254_ECADD_NATIVE_COST};
use crate::system_functions::bytereverse;
use crate::system_functions::curve_hints;
use crypto::ark_ec::CurveGroup;
use crypto::ark_ff::PrimeField;
use crypto::ark_serialize::{CanonicalSerialize, Valid};
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
    O: IOOracle,
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

    let coordinates = buffer.as_chunks_mut::<64>().0.try_into().unwrap();

    let serialized_result =
        bn254_ecadd_inner(coordinates, oracle).map_err(|_| -> SubsystemError<_> {
            interface_error!(Bn254AddInterfaceError::InvalidPoint)
        })?;

    dst.try_extend(serialized_result)
        .map_err(|_| out_of_return_memory!())?;

    Ok(())
}

/// With an oracle, the inversion that makes the result affine comes from a checked hint.
pub fn bn254_ecadd_inner<O: IOOracle>(
    coordinates: &mut [[u8; 64]; 2],
    oracle: Option<&mut O>,
) -> Result<[u8; 64], ()> {
    use crypto::ark_ff::PrimeField;
    use crypto::ark_serialize::CanonicalDeserialize;
    use crypto::bn254::*;

    let mut points = [G1Affine::identity(); 2];
    for (dst, xy) in points.iter_mut().zip(coordinates.iter_mut()) {
        let is_zero = xy.iter().all(|el| *el == 0);
        if is_zero {
            continue;
        }
        // the coordinates are parsed in place: big-endian in the input, little-endian for
        // the deserialization
        let (x, y) = xy.split_at_mut(32);
        bytereverse(x);
        bytereverse(y);
        let x_bigint = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&*x).map_err(|_| ())?;
        let y_bigint = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&*y).map_err(|_| ())?;
        let x_coordinate = Fq::from_bigint(x_bigint).ok_or(())?;
        let y_coordinate = Fq::from_bigint(y_bigint).ok_or(())?;
        let affine_point = G1Affine::new_unchecked(x_coordinate, y_coordinate);
        affine_point.check().map_err(|_| ())?;
        *dst = affine_point;
    }

    let [a, b] = &points;
    // the in-place group law: arkworks' `Projective + Affine` copies every field element
    let result = crypto::bn254::g1::add_affine(a, b);
    let result = serialize_projective(result, oracle);

    Ok(result)
}

/// The EVM encoding of `point`. With an oracle, the inversion of its `Z` coordinate comes from
/// a checked hint instead of an exponentiation.
pub(crate) fn serialize_projective<O: IOOracle>(
    point: crypto::bn254::G1Projective,
    oracle: Option<&mut O>,
) -> [u8; 64] {
    use crypto::ark_ec::AffineRepr;
    use crypto::ark_ff::Zero;
    if point.is_zero() {
        // canonical for zero point
        [0u8; 64]
    } else {
        let result = match oracle {
            Some(oracle) => crypto::hinted_ops::to_affine_with_inverse(&point, |z| {
                curve_hints::bn254_fq_inverse(oracle, z)
            }),
            None => point.into_affine(),
        };
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

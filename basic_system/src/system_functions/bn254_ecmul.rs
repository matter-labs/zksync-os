use super::*;
use crate::cost_constants::BN254_ECMUL_NATIVE_COST;
use crate::{
    cost_constants::BN254_ECMUL_COST_GAS,
    system_functions::bn254_ecadd::{
        bigint_from_be, parse_affine, serialize_affine, serialize_projective,
    },
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
/// With `USE_ADVICE`, the multiplication works in affine coordinates, with its field divisions
/// taken from oracle hints.
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

/// With an oracle, the multiplication works in affine coordinates, with its field divisions
/// from checked hints.
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

    let result = match oracle {
        // affine coordinates, with every division a checked hint
        Some(oracle) => serialize_affine(&crypto::bn254::g1::mul_affine_with_divider(
            &affine_point,
            &scalar.0,
            &mut curve_hints::Bn254FqDivider::new(oracle),
        )),
        None => serialize_projective::<O>(affine_point.mul_bigint(&scalar), None),
    };

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system_functions::bn254_ecadd::bn254_ecadd_inner;
    use callable_oracles::field_hints::NativeFieldOpsQuery;
    use crypto::ark_ec::{AffineRepr, CurveGroup};
    use crypto::ark_ff::PrimeField;
    use oracle_provider::ZkEENonDeterminismSource;

    fn oracle() -> ZkEENonDeterminismSource {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(NativeFieldOpsQuery);
        oracle
    }

    fn be(value: crypto::bn254::Fq) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for (chunk, limb) in bytes
            .as_chunks_mut::<8>()
            .0
            .iter_mut()
            .zip(value.into_bigint().as_ref().iter().rev())
        {
            *chunk = limb.to_be_bytes();
        }
        bytes
    }

    /// Encoded points: the point at infinity, multiples of the generator and their negations
    fn points() -> Vec<[u8; 64]> {
        let generator = crypto::bn254::G1Affine::generator();
        let mut points = vec![[0u8; 64]];
        let mut p = generator.into_group();
        for _ in 0..4 {
            for point in [p.into_affine(), (-p).into_affine()] {
                let mut encoded = [0u8; 64];
                encoded[..32].copy_from_slice(&be(point.x));
                encoded[32..].copy_from_slice(&be(point.y));
                points.push(encoded);
            }
            p += p + generator;
        }
        points
    }

    fn scalars() -> Vec<[u8; 32]> {
        use crypto::ark_ff::{BigInteger, Field};
        let order: [u8; 32] = <crypto::bn254::Fr as PrimeField>::MODULUS
            .to_bytes_be()
            .try_into()
            .unwrap();
        let mut scalars = vec![[0u8; 32], [0xff; 32], order];
        for delta in [1u8, 2] {
            let mut below = order;
            below[31] -= delta;
            scalars.push(below);
            let mut above = order;
            above[31] += delta;
            scalars.push(above);
            let mut small = [0u8; 32];
            small[31] = delta;
            scalars.push(small);
        }
        let mut k = crypto::bn254::Fr::from(0x9e37_79b9_7f4a_7c15u64);
        for _ in 0..12 {
            k = k.square() + crypto::bn254::Fr::from(3u64);
            scalars.push(k.into_bigint().to_bytes_be().try_into().unwrap());
        }
        scalars
    }

    /// With the advice of the oracle (affine arithmetic with hinted divisions) the results are
    /// the ones without it
    #[test]
    fn multiplication_with_advice_matches() {
        for point in points() {
            let (x, y) = (
                point[..32].try_into().unwrap(),
                point[32..].try_into().unwrap(),
            );
            for scalar in scalars() {
                let expected = bn254_ecmul_inner::<ZkEENonDeterminismSource>(x, y, &scalar, None);
                let hinted = bn254_ecmul_inner(x, y, &scalar, Some(&mut oracle()));
                assert!(expected.is_ok());
                assert_eq!(hinted, expected);
            }
        }
    }

    #[test]
    fn addition_with_advice_matches() {
        for a in points() {
            for b in points() {
                let expected = bn254_ecadd_inner::<ZkEENonDeterminismSource>(&[a, b], None);
                let hinted = bn254_ecadd_inner(&[a, b], Some(&mut oracle()));
                assert!(expected.is_ok());
                assert_eq!(hinted, expected);
            }
        }
    }

    #[test]
    fn points_off_the_curve_are_rejected() {
        let mut point = points()[1];
        point[63] ^= 1;
        let (x, y) = (
            point[..32].try_into().unwrap(),
            point[32..].try_into().unwrap(),
        );
        assert!(bn254_ecmul_inner(x, y, &[1; 32], Some(&mut oracle())).is_err());
        assert!(bn254_ecadd_inner(&[point, points()[3]], Some(&mut oracle())).is_err());
    }
}

use super::super::tests as bls_tests;
use super::*;
use crate::cost_constants::BLS12_381_G1MSM_PER_POINT_NATIVE_COST;
use crypto::ark_ec::scalar_mul::sw_double_and_add_affine;
use crypto::ark_ec::{AffineRepr, CurveGroup};
use crypto::ark_ff::PrimeField;
use crypto::ark_ff::{AdditiveGroup, Zero};
use num_bigint::BigUint;
use std::alloc::Global;
use zk_ee::common_traits::TryExtend;
use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
use zk_ee::system::errors::runtime::{FatalRuntimeError, RuntimeError};
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::{Computational, Ergs, ErgsResource, Resource};

type TestResources = BaseResources<DecreasingNative>;
type Scalar = <Fr as PrimeField>::BigInt;

/// The absolute value of the curve parameter
const X: u64 = 0xd201000000010000;

/// Not a point of the curve: `(1, 1)`, and `1 != 1 + 4`
const NOT_ON_CURVE: [u8; G1_SERIALIZATION_LEN] = {
    let mut bytes = [0u8; G1_SERIALIZATION_LEN];
    bytes[FIELD_ELEMENT_SERIALIZATION_LEN - 1] = 1;
    bytes[G1_SERIALIZATION_LEN - 1] = 1;
    bytes
};

/// splitmix64, for deterministic test values
struct TestRng(u64);

impl TestRng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    /// A random integer below `2^bits`, for `bits <= 256`
    fn below_power_of_two(&mut self, bits: u32) -> BigUint {
        let bytes: Vec<u8> = (0..4).flat_map(|_| self.next_u64().to_le_bytes()).collect();
        BigUint::from_bytes_le(&bytes) % (BigUint::from(1u32) << bits)
    }

    fn fq(&mut self) -> Fq {
        let bytes: Vec<u8> = (0..8).flat_map(|_| self.next_u64().to_le_bytes()).collect();
        Fq::from_le_bytes_mod_order(&bytes)
    }
}

fn group_order() -> BigUint {
    <Fr as PrimeField>::MODULUS.into()
}

fn scalar(n: &BigUint) -> Scalar {
    assert!(n.bits() <= 256);
    let mut limbs = [0u64; 4];
    for (limb, digit) in limbs.iter_mut().zip(n.iter_u64_digits()) {
        *limb = digit;
    }
    crypto::BigInt::new(limbs)
}

/// Scalars around the multiples of the group order `r`, `(r ± 1) / 2`, `X`, the extremes of
/// 256 bits, and random ones: full-width (about half of them at least `r`), 50-bit, and
/// below `2^254 < r`
fn test_scalars(rng: &mut TestRng) -> Vec<Scalar> {
    let r = group_order();
    let one = BigUint::from(1u32);
    let mut scalars: Vec<BigUint> = vec![
        BigUint::from(0u32),
        one.clone(),
        BigUint::from(2u32),
        BigUint::from(3u32),
        &r - 1u32,
        r.clone(),
        &r + 1u32,
        &r * 2u32,
        &r * 2u32 + 1u32,
        &one << 255u32,
        (&one << 256u32) - 1u32,
        BigUint::from(X),
        (&r - 1u32) / 2u32,
        (&r + 1u32) / 2u32,
    ];
    scalars.extend((0..24).map(|_| rng.below_power_of_two(256)));
    scalars.extend((0..8).map(|_| rng.below_power_of_two(50)));
    scalars.extend((0..8).map(|_| rng.below_power_of_two(254)));
    scalars.iter().map(scalar).collect()
}

/// The point at infinity, the generator, and `count` random points of G1, multiplied out by
/// a plain double-and-add (not by the GLV multiplication under test)
fn test_points(rng: &mut TestRng, count: usize) -> Vec<G1Affine> {
    let mut points = vec![G1Affine::identity(), G1Affine::generator()];
    for _ in 0..count {
        let k = scalar(&rng.below_power_of_two(255));
        points.push(sw_double_and_add_affine(&G1Affine::generator(), k).into_affine());
    }
    points
}

fn encode_point(p: G1Affine) -> [u8; G1_SERIALIZATION_LEN] {
    let mut bytes = [0u8; G1_SERIALIZATION_LEN];
    crypto::bls12_381::eip2537::serialize_g1_bytes(p, &mut bytes);
    bytes
}

fn encode_pair(point: &[u8; G1_SERIALIZATION_LEN], s: &Scalar) -> Vec<u8> {
    let mut input = point.to_vec();
    for limb in s.0.iter().rev() {
        input.extend_from_slice(&limb.to_be_bytes());
    }
    input
}

/// The resources left after the charge for one pair: the per-point costs, with no discount
fn charged_for_one_pair() -> TestResources {
    let mut resources = TestResources::FORMAL_INFINITE;
    resources
        .charge_legacy_gas_and_native(
            BLS12_381_G1_MSM_PER_POINT_GAS,
            BLS12_381_G1MSM_PER_POINT_NATIVE_COST,
        )
        .unwrap();
    resources
}

fn execute(
    input: &[u8],
    resources: &mut TestResources,
) -> (Result<(), SubsystemError<Bls12PrecompileErrors>>, Vec<u8>) {
    let mut output = Vec::new();
    let mut hinted_resources = resources.clone();
    let mut hinted_output = Vec::new();
    let hinted = Bls12381G1MSMPrecompile::<true>::execute(
        input,
        &mut hinted_output,
        &mut hinted_resources,
        &mut bls_tests::honest_oracle(),
        &mut zk_ee::system::logger::NullLogger,
        Global,
    );
    let result = Bls12381G1MSMPrecompile::<false>::execute(
        input,
        &mut output,
        resources,
        &mut bls_tests::Unqueried,
        &mut zk_ee::system::logger::NullLogger,
        Global,
    );
    assert_eq!(format!("{result:?}"), format!("{hinted:?}"));
    assert_eq!(output, hinted_output);
    assert_eq!(*resources, hinted_resources);
    (result, output)
}

/// The pair is rejected with `expected` after the charge, and nothing is written
fn assert_rejected(
    point: &[u8; G1_SERIALIZATION_LEN],
    s: &Scalar,
    expected: Bls12PrecompileInterfaceError,
) {
    let mut resources = TestResources::FORMAL_INFINITE;
    let (result, output) = execute(&encode_pair(point, s), &mut resources);
    let err = result.expect_err("the pair must be rejected");
    assert!(
        matches!(&err, SubsystemError::LeafUsage(err) if err.0 == expected),
        "{err:?}"
    );
    assert!(output.is_empty());
    assert_eq!(resources, charged_for_one_pair());
}

mod errors;
mod multiple_pairs;
mod single_pair;

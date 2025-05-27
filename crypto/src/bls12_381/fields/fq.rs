#[cfg(all(target_arch = "riscv32", not(feature = "bigint_ops")))]
compile_error!("feature `bigint_ops` must be activated for RISC-V target");

// partially reused cargo expand of derived FqConfig with multiplication updated

// Prime modulus is 4002409555221667393417789825735904156556882819939007885332058136124031650490837864442687629129015664037894272559787

// NOTE: we operate with 256-bit "limbs", so Montgomery representation is 512 bits

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub fn init() {
    unsafe {
        MODULUS.as_mut_ptr().write(MODULUS_CONSTANT);
        REDUCTION_CONST.as_mut_ptr().write(MONT_REDUCTION_CONSTANT);
    }
}

#[derive(Default)]
struct FqParams;

impl DelegatedModParams<8> for FqParams {
    unsafe fn modulus() -> &'static BigInt<8> {
        unsafe { MODULUS.assume_init_ref() }
    }
}

impl DelegatedMontParams<8> for FqParams {
    unsafe fn reduction_const() -> &'static BigInt<4> {
        unsafe { REDUCTION_CONST.assume_init_ref() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FqConfig;

const NUM_LIMBS: usize = 8usize;

pub type Fq = Fp512<MontBackend<FqConfig, NUM_LIMBS>>;

use crate::ark_ff_delegation::{BigInt, BigIntMacro, Fp, Fp512, MontBackend, MontConfig};
use crate::bigint_delegation::{u512, DelegatedModParams, DelegatedMontParams};
use ark_ff::{AdditiveGroup, Field, Zero};
use core::mem::MaybeUninit;

type B = BigInt<NUM_LIMBS>;
type F = Fp<MontBackend<FqConfig, NUM_LIMBS>, NUM_LIMBS>;

// we also need few empty representations

static mut MODULUS: MaybeUninit<BigInt<8>> = MaybeUninit::uninit();
static mut REDUCTION_CONST: MaybeUninit<BigInt<4>> = MaybeUninit::uninit();

const MODULUS_CONSTANT: BigInt<8> = BigIntMacro!("4002409555221667393417789825735904156556882819939007885332058136124031650490837864442687629129015664037894272559787");
// it's - MODULUS^-1 mod 2^256
const MONT_REDUCTION_CONSTANT: BigInt<4> =
    BigIntMacro!("11726191667098586211898467594267748916577995138249226639719947807923487178749");

// a^-1 = a ^ (p - 2)
const INVERSION_POW: [u64; 6] = [
    13402431016077863595u64 - 2,
    2210141511517208575u64,
    7435674573564081700u64,
    7239337960414712511u64,
    5412103778470702295u64,
    1873798617647539866u64,
];

// NOTE: even though we pretend to be u64 everywhere, on LE machine (and our RISC-V 32IM is such) we do not care
// for purposes of our precompile calls

impl MontConfig<NUM_LIMBS> for FqConfig {
    const MODULUS: B = BigInt([
        13402431016077863595u64,
        2210141511517208575u64,
        7435674573564081700u64,
        7239337960414712511u64,
        5412103778470702295u64,
        1873798617647539866u64,
        0,
        0,
    ]);

    // we also need to override into_bigint to properly perform
    // conversion
    fn into_bigint(mut a: Fp<MontBackend<Self, NUM_LIMBS>, NUM_LIMBS>) -> BigInt<NUM_LIMBS> {
        // for now it's just a multiplication with 1 literal
        unsafe {
            u512::mul_assign_montgomery::<FqParams>(&mut a.0, &BigInt::one());
        }
        a.0
    }

    const GENERATOR: F = {
        let (is_positive, limbs) = (true, [2u64]);
        Fp::from_sign_and_limbs(is_positive, &limbs)
    };
    const TWO_ADIC_ROOT_OF_UNITY: F = {
        let (is_positive, limbs) = (
            true,
            [
                13402431016077863594u64,
                2210141511517208575u64,
                7435674573564081700u64,
                7239337960414712511u64,
                5412103778470702295u64,
                1873798617647539866u64,
            ],
        );
        Fp::from_sign_and_limbs(is_positive, &limbs)
    };
    const SMALL_SUBGROUP_BASE: Option<u32> = Some(3u32);
    const SMALL_SUBGROUP_BASE_ADICITY: Option<u32> = Some(2u32);
    const LARGE_SUBGROUP_ROOT_OF_UNITY: Option<F> = Some({
        let (is_positive, limbs) = (
            true,
            [
                5896295325348737640u64,
                5503863413011229930u64,
                11466573396089897971u64,
                17103254516989687468u64,
                7243505556163372831u64,
                1399342764408159943u64,
            ],
        );
        Fp::from_sign_and_limbs(is_positive, &limbs)
    });

    #[inline(always)]
    fn add_assign(a: &mut F, b: &F) {
        unsafe {
            u512::add_mod_assign::<FqParams>(&mut a.0, &b.0);
        }
    }
    #[inline(always)]
    fn sub_assign(a: &mut F, b: &F) {
        unsafe {
            u512::sub_mod_assign::<FqParams>(&mut a.0, &b.0);
        }
    }

    #[inline(always)]
    fn double_in_place(a: &mut F) {
        unsafe {
            u512::double_mod_assign::<FqParams>(&mut a.0);
        }
    }
    /// Sets `a = -a`.
    #[inline(always)]
    fn neg_in_place(a: &mut F) {
        unsafe {
            u512::neg_mod_assign::<FqParams>(&mut a.0);
        }
    }

    #[inline(always)]
    fn mul_assign(a: &mut F, b: &F) {
        unsafe {
            u512::mul_assign_montgomery::<FqParams>(&mut a.0, &b.0);
        }
    }

    #[inline(always)]
    fn square_in_place(a: &mut F) {
        unsafe {
            u512::square_assign_montgomery::<FqParams>(&mut a.0);
        }
    }

    fn inverse(
        a: &Fp<MontBackend<Self, NUM_LIMBS>, NUM_LIMBS>,
    ) -> Option<Fp<MontBackend<Self, NUM_LIMBS>, NUM_LIMBS>> {
        if a.is_zero() {
            return None;
        }

        let inverse = a.pow(INVERSION_POW);

        Some(inverse)
    }

    // default impl
    fn sum_of_products<const M: usize>(a: &[F; M], b: &[F; M]) -> F {
        let mut sum = F::ZERO;
        for i in 0..a.len() {
            sum += a[i] * b[i];
        }
        sum
    }
}

#[cfg(test)]
mod test {
    use super::{BigInt, Fq, FqConfig, MontConfig, B};
    use ark_ff::{Field, One, UniformRand, Zero};

    fn init() {
        crate::bls12_381::fields::init();
        crate::bigint_delegation::init();
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_mul_compare() {
        const ITERATIONS: usize = 100000;
        init();

        let one_bigint = BigInt::one();
        let t = Fq::from_bigint(one_bigint).unwrap();
        assert_eq!(t.0, FqConfig::R);

        use ark_std::test_rng;
        let mut rng = test_rng();

        type RefFq = ark_bls12_381::Fq;

        for i in 0..ITERATIONS {
            let ref_a = RefFq::rand(&mut rng);
            let ref_b = RefFq::rand(&mut rng);

            let mut t = BigInt::zero();
            t.0[..6].copy_from_slice(&ref_a.into_bigint().0);
            let a = Fq::from_bigint(t).unwrap();
            let mut t = BigInt::zero();
            t.0[..6].copy_from_slice(&ref_b.into_bigint().0);
            let b = Fq::from_bigint(t).unwrap();

            assert_eq!(ref_a.into_bigint().0[..6], a.into_bigint().0[..6]);
            assert_eq!(ref_b.into_bigint().0[..6], b.into_bigint().0[..6]);

            assert_eq!(
                (ref_a * ref_b).into_bigint().0[..6],
                (a * b).into_bigint().0[..6],
                "failed at iteration {}",
                i,
            );
        }
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_mul_properties() {
        const ITERATIONS: usize = 1000;
        init();

        use ark_std::test_rng;
        let mut rng = test_rng();
        let zero = Fq::zero();
        let one = Fq::one();
        assert_eq!(one.inverse().unwrap(), one, "One inverse failed");
        assert!(one.is_one(), "One is not one");

        assert!(Fq::ONE.is_one(), "One constant is not one");
        assert_eq!(Fq::ONE, one, "One constant is incorrect");

        type RefFq = ark_bls12_381::Fq;

        for _ in 0..ITERATIONS {
            // Associativity
            let ref_a = RefFq::rand(&mut rng);
            let ref_b = RefFq::rand(&mut rng);
            let ref_c = RefFq::rand(&mut rng);

            let a = convert_fq(ref_a);
            let b = convert_fq(ref_b);
            let c = convert_fq(ref_c);
            assert_eq!((a * b) * c, a * (b * c), "Associativity failed");

            // Commutativity
            assert_eq!(a * b, b * a, "Commutativity failed");

            // Identity
            assert_eq!(one * a, a, "Identity mul failed");
            assert_eq!(one * b, b, "Identity mul failed");
            assert_eq!(one * c, c, "Identity mul failed");

            assert_eq!(zero * a, zero, "Mul by zero failed");
            assert_eq!(zero * b, zero, "Mul by zero failed");
            assert_eq!(zero * c, zero, "Mul by zero failed");

            // Inverses
            assert_eq!(a * a.inverse().unwrap(), one, "Mul by inverse failed");
            assert_eq!(b * b.inverse().unwrap(), one, "Mul by inverse failed");
            assert_eq!(c * c.inverse().unwrap(), one, "Mul by inverse failed");

            // Associativity and commutativity simultaneously
            let t0 = (a * b) * c;
            let t1 = (a * c) * b;
            let t2 = (b * c) * a;
            assert_eq!(t0, t1, "Associativity + commutativity failed");
            assert_eq!(t1, t2, "Associativity + commutativity failed");

            // Squaring
            assert_eq!(a * a, a.square(), "Squaring failed");
            assert_eq!(b * b, b.square(), "Squaring failed");
            assert_eq!(c * c, c.square(), "Squaring failed");

            // Distributivity
            assert_eq!(a * (b + c), a * b + a * c, "Distributivity failed");
            assert_eq!(b * (a + c), b * a + b * c, "Distributivity failed");
            assert_eq!(c * (a + b), c * a + c * b, "Distributivity failed");
            assert_eq!(
                (a + b).square(),
                a.square() + b.square() + a * ark_ff::AdditiveGroup::double(&b),
                "Distributivity for square failed"
            );
            assert_eq!(
                (b + c).square(),
                c.square() + b.square() + c * ark_ff::AdditiveGroup::double(&b),
                "Distributivity for square failed"
            );
            assert_eq!(
                (c + a).square(),
                a.square() + c.square() + a * ark_ff::AdditiveGroup::double(&c),
                "Distributivity for square failed"
            );
        }
    }

    // NOTE: those tests are backported as we need to init static and run single thread
    // instead of full arkwords test suite. This coverage is ok as our base math is just
    // very small

    pub const ITERATIONS: usize = 100;
    use crate::bls12_381::curves::Bls12_381;
    use ark_bls12_381::Bls12_381 as Bls12_381_Ref;
    use ark_bls12_381::Fq as FqRef;
    use ark_bls12_381::Fq2 as Fq2Ref;
    use ark_bls12_381::Fq6 as Fq6Ref;
    use ark_ec::{pairing::*, CurveGroup, PrimeGroup};
    use ark_ff::{CyclotomicMultSubgroup, PrimeField};
    use ark_std::test_rng;

    fn convert_fq(src: FqRef) -> Fq {
        let mut t = B::zero();
        t.0[..6].copy_from_slice(&src.into_bigint().0);

        Fq::from_bigint(t).unwrap()
    }

    fn convert_fq2(src: Fq2Ref) -> super::super::Fq2 {
        super::super::Fq2 {
            c0: convert_fq(src.c0),
            c1: convert_fq(src.c1),
        }
    }

    fn convert_g1(src: <Bls12_381_Ref as Pairing>::G1) -> <Bls12_381 as Pairing>::G1 {
        crate::bls12_381::G1Projective {
            x: convert_fq(src.x),
            y: convert_fq(src.y),
            z: convert_fq(src.z),
        }
    }

    fn convert_g2(src: <Bls12_381_Ref as Pairing>::G2) -> <Bls12_381 as Pairing>::G2 {
        crate::bls12_381::G2Projective {
            x: convert_fq2(src.x),
            y: convert_fq2(src.y),
            z: convert_fq2(src.z),
        }
    }

    fn convert_g1_affine(
        src: <Bls12_381_Ref as Pairing>::G1Affine,
    ) -> <Bls12_381 as Pairing>::G1Affine {
        crate::bls12_381::G1Affine {
            x: convert_fq(src.x),
            y: convert_fq(src.y),
            infinity: src.infinity,
        }
    }

    fn convert_g2_affine(
        src: <Bls12_381_Ref as Pairing>::G2Affine,
    ) -> <Bls12_381 as Pairing>::G2Affine {
        crate::bls12_381::G2Affine {
            x: convert_fq2(src.x),
            y: convert_fq2(src.y),
            infinity: src.infinity,
        }
    }

    fn convert_fq6(src: Fq6Ref) -> crate::bls12_381::Fq6 {
        crate::bls12_381::Fq6 {
            c0: convert_fq2(src.c0),
            c1: convert_fq2(src.c1),
            c2: convert_fq2(src.c2),
        }
    }

    fn convert_fq12(
        src: <Bls12_381_Ref as Pairing>::TargetField,
    ) -> <Bls12_381 as Pairing>::TargetField {
        crate::bls12_381::Fq12 {
            c0: convert_fq6(src.c0),
            c1: convert_fq6(src.c1),
        }
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_bilinearity() {
        init();
        for _ in 0..100 {
            let mut rng = test_rng();
            let a: <Bls12_381_Ref as Pairing>::G1 = UniformRand::rand(&mut rng);
            let b: <Bls12_381_Ref as Pairing>::G2 = UniformRand::rand(&mut rng);
            let s: <Bls12_381 as Pairing>::ScalarField = UniformRand::rand(&mut rng);

            let a = convert_g1(a);
            let b = convert_g2(b);

            let sa = a * s;
            let sb = b * s;

            let ans1 = <Bls12_381>::pairing(sa, b);
            let ans2 = <Bls12_381>::pairing(a, sb);
            let ans3 = <Bls12_381>::pairing(a, b) * s;

            assert_eq!(ans1, ans2);
            assert_eq!(ans2, ans3);

            assert_ne!(ans1, PairingOutput::zero());
            assert_ne!(ans2, PairingOutput::zero());
            assert_ne!(ans3, PairingOutput::zero());
            let group_order = <<Bls12_381 as Pairing>::ScalarField>::characteristic();

            assert_eq!(ans1.mul_bigint(group_order), PairingOutput::zero());
            assert_eq!(ans2.mul_bigint(group_order), PairingOutput::zero());
            assert_eq!(ans3.mul_bigint(group_order), PairingOutput::zero());
        }
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_multi_pairing() {
        init();
        for _ in 0..ITERATIONS {
            let rng = &mut test_rng();

            let a = <Bls12_381_Ref as Pairing>::G1::rand(rng).into_affine();
            let b = <Bls12_381_Ref as Pairing>::G2::rand(rng).into_affine();
            let c = <Bls12_381_Ref as Pairing>::G1::rand(rng).into_affine();
            let d = <Bls12_381_Ref as Pairing>::G2::rand(rng).into_affine();

            let a = convert_g1_affine(a);
            let b = convert_g2_affine(b);
            let c = convert_g1_affine(c);
            let d = convert_g2_affine(d);

            let ans1 = <Bls12_381>::pairing(a, b) + &<Bls12_381>::pairing(c, d);
            let ans2 = <Bls12_381>::multi_pairing(&[a, c], &[b, d]);
            assert_eq!(ans1, ans2);
        }
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_final_exp() {
        init();
        for _ in 0..ITERATIONS {
            let rng = &mut test_rng();
            let fp_ext = <Bls12_381_Ref as Pairing>::TargetField::rand(rng);
            let fp_ext = convert_fq12(fp_ext);
            let gt = <Bls12_381 as Pairing>::final_exponentiation(MillerLoopOutput(fp_ext))
                .unwrap()
                .0;
            let r = <Bls12_381 as Pairing>::ScalarField::MODULUS;
            assert!(gt.cyclotomic_exp(r).is_one());
        }
    }
}

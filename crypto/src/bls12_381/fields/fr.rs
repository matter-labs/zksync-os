#[cfg(all(target_arch = "riscv32", not(feature = "bigint_ops")))]
compile_error!("feature `bigint_ops` must be activated for RISC-V target");

use core::mem::MaybeUninit;

use crate::ark_ff_delegation::{BigInt, BigIntMacro, Fp, Fp256, MontBackend, MontConfig};
use crate::bigint_arithmatic::u256::*;
use ark_ff::{AdditiveGroup, Zero};
use bigint_riscv::DelegatedU256;

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub fn init() {
    unsafe {
        MODULUS.as_mut_ptr().write(MODULUS_CONSTANT);
        REDUCTION_CONST.as_mut_ptr().write(MONT_REDUCTION_CONSTANT);
    }
}

static mut MODULUS: MaybeUninit<DelegatedU256> = MaybeUninit::uninit();
static mut REDUCTION_CONST: MaybeUninit<DelegatedU256> = MaybeUninit::uninit();

const MONT_REDUCTION_CONSTANT_LIMBS: [u64; 4] =
    BigIntMacro!("27711634432943687283656245953990505159342029877880134060146103271536583507967").0;

const MONT_REDUCTION_CONSTANT: DelegatedU256 =
    DelegatedU256::from_limbs(MONT_REDUCTION_CONSTANT_LIMBS);
const MODULUS_CONSTANT: DelegatedU256 = DelegatedU256::from_limbs(FrConfig::MODULUS.0);

#[derive(Default, Debug)]
pub struct FrParams;

impl DelegatedModParams for FrParams {
    unsafe fn modulus() -> &'static DelegatedU256 {
        unsafe { MODULUS.assume_init_ref() }
    }
}

impl DelegatedMontParams for FrParams {
    unsafe fn reduction_const() -> &'static DelegatedU256 {
        unsafe { REDUCTION_CONST.assume_init_ref() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrConfig;

pub type Fr = Fp256<MontBackend<FrConfig, 4>>;

impl MontConfig<4> for FrConfig {
    const MODULUS: BigInt<4> = BigIntMacro!(
        "52435875175126190479447740508185965837690552500527637822603658699938581184513"
    );

    const GENERATOR: Fr = {
        let (is_positive, limbs) = (true, [7u64]);
        Fp::from_sign_and_limbs(is_positive, &limbs)
    };

    const TWO_ADIC_ROOT_OF_UNITY: Fr = {
        let (is_positive, limbs) = (
            true,
            [
                4046931900703378731,
                13129826145616953529,
                15031722638446171060,
                1631043718794977056,
            ],
        );
        Fp::from_sign_and_limbs(is_positive, &limbs)
    };

    const SMALL_SUBGROUP_BASE: Option<u32> = Some(3u32);
    const SMALL_SUBGROUP_BASE_ADICITY: Option<u32> = Some(1);
    const LARGE_SUBGROUP_ROOT_OF_UNITY: Option<Fr> = Some({
        let (is_positive, limbs) = (
            true,
            [
                196249104034986263,
                9632877624223158608,
                16881125688358416649,
                4331619260936696776,
            ],
        );
        Fr::from_sign_and_limbs(is_positive, &limbs)
    });

    fn into_bigint(mut a: Fr) -> BigInt<4> {
        unsafe {
            mul_assign_montgomery::<FrParams>(from_ark_mut(&mut a.0), &DelegatedU256::one());
        }
        a.0
    }

    #[inline(always)]
    fn add_assign(a: &mut Fr, b: &Fr) {
        unsafe {
            add_mod_assign::<FrParams>(from_ark_mut(&mut a.0), from_ark_ref(&b.0));
        }
    }

    #[inline(always)]
    fn sub_assign(a: &mut Fr, b: &Fr) {
        unsafe {
            sub_mod_assign::<FrParams>(from_ark_mut(&mut a.0), from_ark_ref(&b.0));
        }
    }

    #[inline(always)]
    fn double_in_place(a: &mut Fr) {
        unsafe {
            double_mod_assign::<FrParams>(from_ark_mut(&mut a.0));
        }
    }

    #[inline(always)]
    fn neg_in_place(a: &mut Fr) {
        unsafe {
            neg_mod_assign::<FrParams>(from_ark_mut(&mut a.0));
        }
    }

    #[inline(always)]
    fn mul_assign(a: &mut Fr, b: &Fr) {
        unsafe {
            mul_assign_montgomery::<FrParams>(from_ark_mut(&mut a.0), from_ark_ref(&b.0));
        }
    }

    #[inline(always)]
    fn square_in_place(a: &mut Fr) {
        unsafe {
            square_assign_montgomery::<FrParams>(from_ark_mut(&mut a.0));
        }
    }

    fn inverse(a: &Fr) -> Option<Fr> {
        __gcd_inverse(a)
    }

    fn sum_of_products<const M: usize>(a: &[Fr; M], b: &[Fr; M]) -> Fr {
        let mut sum = Fr::ZERO;
        for i in 0..M {
            sum += a[i] * &b[i]
        }
        sum
    }
}

fn __gcd_inverse(a: &Fr) -> Option<Fr> {
    if a.is_zero() {
        return None;
    }
    // Guajardo Kumar Paar Pelzl
    // Efficient Software-Implementation of Finite Fields with Applications to
    // Cryptography
    // Algorithm 16 (BEA for Inversion in Fp)

    use ark_ff::BigInteger;
    use ark_ff::PrimeField;

    let mut u = a.0;
    let mut v = Fr::MODULUS;
    let mut b = Fp::new_unchecked(Fr::R2); // Avoids unnecessary reduction step.
    let mut c = Fp::zero();
    let modulus = Fr::MODULUS;

    while !from_ark_ref(&u).is_one() && !from_ark_ref(&v).is_one() {
        while u.is_even() {
            u.div2();

            if b.0.is_even() {
                b.0.div2();
            } else {
                let _carry = from_ark_mut(&mut b.0).overflowing_add_assign(from_ark_ref(&modulus));
                b.0.div2();
                // if !Self::MODULUS_HAS_SPARE_BIT && carry {
                //     (b.0).0[N - 1] |= 1 << 63;
                // }
            }
        }

        while v.is_even() {
            v.div2();

            if c.0.is_even() {
                c.0.div2();
            } else {
                let _carry = from_ark_mut(&mut c.0).overflowing_add_assign(from_ark_ref(&modulus));
                c.0.div2();
                // if !Self::MODULUS_HAS_SPARE_BIT && carry {
                //     (c.0).0[N - 1] |= 1 << 63;
                // }
            }
        }

        // if v < u {
        if v.lt(&u) {
            from_ark_mut(&mut u).overflowing_sub_assign(from_ark_ref(&v));
            b -= &c;
        } else {
            from_ark_mut(&mut v).overflowing_sub_assign(from_ark_ref(&u));
            c -= &b;
        }
    }

    if from_ark_ref(&u).is_one() {
        Some(b)
    } else {
        Some(c)
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for Fr {
    type Parameters = ();

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        use ark_ff::PrimeField;
        use proptest::prelude::{any, Strategy};
        any::<U256Wrapper<FrParams>>().prop_map(|x| {
            let bigint = BigInt::<4>(*x.0.as_limbs());
            Self::from_bigint(bigint).unwrap()
        })
    }

    type Strategy = proptest::arbitrary::Mapped<U256Wrapper<FrParams>, Self>;
}

#[cfg(test)]
mod tests {
    use super::Fr;
    use ark_ff::{AdditiveGroup, Field, Zero};
    use proptest::{prop_assert_eq, proptest};
    fn init() {
        super::init();
        bigint_riscv::init();
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_inverse_properties() {
        init();
        proptest!(|(x: Fr)| {
            if !x.is_zero() {
                prop_assert_eq!(x.inverse().unwrap().inverse().unwrap(), x);
                prop_assert_eq!(x.inverse().unwrap() * x, Fr::ONE);
            } else {
                prop_assert_eq!(x.inverse(), None);
            }
        })
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_mul_properties() {
        init();
        proptest!(|(x: Fr, y: Fr, z: Fr)| {
            prop_assert_eq!(x * y, y * x);
            prop_assert_eq!((x * y) * z, x * (y * z));
            prop_assert_eq!(x * Fr::ONE, x);
            prop_assert_eq!(x * Fr::ZERO, Fr::ZERO);
            prop_assert_eq!(x * (y + z), x * y + x * z);
        })
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_add_properties() {
        init();
        proptest!(|(x: Fr, y: Fr, z: Fr)| {
            prop_assert_eq!(x + y, y + x);
            prop_assert_eq!(x + Fr::ZERO, x);
            prop_assert_eq!((x + y) + z, x + (y + z));
            prop_assert_eq!(x - x, Fr::ZERO);
            prop_assert_eq!((x + y) - y, x);
            prop_assert_eq!((x - y) + y, x);
        })
    }
}

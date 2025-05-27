#[cfg(all(target_arch = "riscv32", not(feature = "bigint_ops")))]
compile_error!("feature `bigint_ops` must be activated for RISC-V target");

use core::mem::MaybeUninit;

use crate::ark_ff_delegation::{BigInt, BigIntMacro, Fp, Fp256, MontBackend, MontConfig};
use crate::bigint_delegation::{u256, DelegatedModParams, DelegatedMontParams};
use ark_ff::{AdditiveGroup, Zero};

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub fn init() {
    unsafe {
        MODULUS.as_mut_ptr().write(FrConfig::MODULUS);
        REDUCTION_CONST.as_mut_ptr().write(MONT_REDUCTION_CONSTANT);
    }
}

static mut MODULUS: MaybeUninit<BigInt<4>> = MaybeUninit::uninit();
static mut REDUCTION_CONST: MaybeUninit<BigInt<4>> = MaybeUninit::uninit();

const MONT_REDUCTION_CONSTANT: BigInt<4> =
    BigIntMacro!("27711634432943687283656245953990505159342029877880134060146103271536583507967");

#[derive(Default, Debug)]
pub struct FrParams;

impl DelegatedModParams<4> for FrParams {
    unsafe fn modulus() -> &'static BigInt<4> {
        unsafe { MODULUS.assume_init_ref() }
    }
}

impl DelegatedMontParams<4> for FrParams {
    unsafe fn reduction_const() -> &'static BigInt<4> {
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
            u256::mul_assign_montgomery::<FrParams>(&mut a.0, &BigInt::one());
        }
        a.0
    }

    #[inline(always)]
    fn add_assign(a: &mut Fr, b: &Fr) {
        unsafe {
            u256::add_mod_assign::<FrParams>(&mut a.0, &b.0);
        }
    }

    #[inline(always)]
    fn sub_assign(a: &mut Fr, b: &Fr) {
        unsafe {
            u256::sub_mod_assign::<FrParams>(&mut a.0, &b.0);
        }
    }

    #[inline(always)]
    fn double_in_place(a: &mut Fr) {
        unsafe {
            u256::double_mod_assign::<FrParams>(&mut a.0);
        }
    }

    #[inline(always)]
    fn neg_in_place(a: &mut Fr) {
        unsafe {
            u256::neg_mod_assign::<FrParams>(&mut a.0);
        }
    }

    #[inline(always)]
    fn mul_assign(a: &mut Fr, b: &Fr) {
        unsafe {
            u256::mul_assign_montgomery::<FrParams>(&mut a.0, &b.0);
        }
    }

    #[inline(always)]
    fn square_in_place(a: &mut Fr) {
        unsafe {
            u256::square_assign_montgomery::<FrParams>(&mut a.0);
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

    while !u256::is_one(&mut u) && !u256::is_one(&mut v) {
        while u.is_even() {
            u.div2();

            if b.0.is_even() {
                b.0.div2();
            } else {
                let _carry = u256::add_assign(&mut b.0, &modulus);
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
                let _carry = u256::add_assign(&mut c.0, &modulus);
                c.0.div2();
                // if !Self::MODULUS_HAS_SPARE_BIT && carry {
                //     (c.0).0[N - 1] |= 1 << 63;
                // }
            }
        }

        // if v < u {
        if v.lt(&u) {
            u256::sub_assign(&mut u, &v);
            b -= &c;
        } else {
            u256::sub_assign(&mut v, &u);
            c -= &b;
        }
    }

    if u256::is_one(&mut u) {
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
        any::<u256::U256Wrapper<FrParams>>().prop_map(|x| Self::from_bigint(x.0).unwrap())
    }

    type Strategy = proptest::arbitrary::Mapped<u256::U256Wrapper<FrParams>, Self>;
}

#[cfg(test)]
mod tests {
    use super::Fr;
    use ark_ff::{AdditiveGroup, Field, Zero};
    use proptest::{prop_assert_eq, proptest};
    fn init() {
        super::init();
        crate::bigint_delegation::init();
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

#[cfg(all(target_arch = "riscv32", not(feature = "bigint_ops")))]
compile_error!("feature `bigint_ops` must be activated for RISC-V target");

// partially reused cargo expand of derived FqConfig with multiplication updated

// Prime modulus is 4002409555221667393417789825735904156556882819939007885332058136124031650490837864442687629129015664037894272559787

use crate::bigint_riscv::*;

// NOTE: we operate with 256-bit "limbs", so Montgomery representation is 512 bits

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub fn init() {
    unsafe {
        ZERO_REPR_0.as_mut_ptr().write(ZERO_REPR_CONST.0);
        ZERO_REPR_1.as_mut_ptr().write(ZERO_REPR_CONST.0);
        ONE_REPR.as_mut_ptr().write(ONE_REPR_CONST.0);
        MODULUS_REPR_LOW.as_mut_ptr().write(MODULUS_CONSTANT_0);
        MODULUS_REPR_HIGH.as_mut_ptr().write(MODULUS_CONSTANT_1);
        REDUCTION_CONST_REPR
            .as_mut_ptr()
            .write(MONT_REDUCTION_CONSTANT);

        LOW_WORD_SCRATCH.as_mut_ptr().write(ZERO_REPR_CONST.0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FqConfig;

const NUM_LIMBS: usize = 8usize;

pub type Fq = Fp512<MontBackend<FqConfig, NUM_LIMBS>>;

use core::mem::MaybeUninit;

use ark_ff::{AdditiveGroup, BigInt, Field, Fp, Fp512, MontBackend, MontConfig, Zero};

type B = BigInt<NUM_LIMBS>;
type F = Fp<MontBackend<FqConfig, NUM_LIMBS>, NUM_LIMBS>;

// we also need few empty representations

static mut ZERO_REPR_0: MaybeUninit<[u32; 8]> = MaybeUninit::uninit();
static mut ZERO_REPR_1: MaybeUninit<[u32; 8]> = MaybeUninit::uninit();
static mut ONE_REPR: MaybeUninit<[u32; 8]> = MaybeUninit::uninit();
// static mut MINUS_ONE_REPR: MaybeUninit<[u32; 8]> = MaybeUninit::uninit();
static mut MODULUS_REPR_LOW: MaybeUninit<[u32; 8]> = MaybeUninit::uninit();
static mut MODULUS_REPR_HIGH: MaybeUninit<[u32; 8]> = MaybeUninit::uninit();
static mut REDUCTION_CONST_REPR: MaybeUninit<[u32; 8]> = MaybeUninit::uninit();
static mut LOW_WORD_SCRATCH: MaybeUninit<[u32; 8]> = MaybeUninit::uninit();

// lower 256 bits of the modulus
const MODULUS_CONSTANT_0: [u32; 8] = [
    0xffffaaab, 0xb9feffff, 0xb153ffff, 0x1eabfffe, 0xf6b0f624, 0x6730d2a0, 0xf38512bf, 0x64774b84,
];

// higher 256 bits of the modulus
const MODULUS_CONSTANT_1: [u32; 8] = [0x434bacd7, 0x4b1ba7b6, 0x397fe69a, 0x1a0111ea, 0, 0, 0, 0];

// it's - MODULUS^-1 mod 2^256
const MONT_REDUCTION_CONSTANT: [u32; 8] = [
    0xfffcfffd, 0x89f3fffc, 0xd9d113e8, 0x286adb92, 0xc8e30b48, 0x16ef2ef0, 0x8eb2db4c, 0x19ecca0e,
];

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
    fn into_bigint(a: Fp<MontBackend<Self, NUM_LIMBS>, NUM_LIMBS>) -> BigInt<NUM_LIMBS> {
        // for now it's just a multiplication with 1 literal
        let mut a = a;
        __mul_assign_impl(&mut a.0, &BigInt::one());
        __subtract_modulus(&mut a);

        a.0
    }

    const GENERATOR: F = {
        let (is_positive, limbs) = (true, [2u64]);
        ::ark_ff::Fp::from_sign_and_limbs(is_positive, &limbs)
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
        ::ark_ff::Fp::from_sign_and_limbs(is_positive, &limbs)
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
        ::ark_ff::Fp::from_sign_and_limbs(is_positive, &limbs)
    });

    #[inline(always)]
    fn add_assign(a: &mut F, b: &F) {
        __add_with_carry(&mut a.0, &b.0);
        __subtract_modulus(a);
    }
    #[inline(always)]
    fn sub_assign(a: &mut F, b: &F) {
        unsafe {
            let b0: *const [u32; 8] = b.0 .0.as_ptr().cast();
            let b0 = copy_if_needed(b0);

            let borrow =
                bigint_op_delegation::<SUB_OP_BIT_IDX>(a.0 .0.as_mut_ptr().cast(), b0.cast());

            let b1: *const [u32; 8] = b.0 .0.as_ptr().cast::<[u32; 8]>().add(1);
            let b1 = copy_if_needed(b1);

            let borrow = bigint_op_delegation_with_carry_bit::<SUB_OP_BIT_IDX>(
                a.0 .0.as_mut_ptr().cast::<[u32; 8]>().add(1).cast(),
                b1.cast(),
                borrow != 0,
            );

            if borrow != 0 {
                // we should add modulus
                let carry = bigint_op_delegation::<ADD_OP_BIT_IDX>(
                    a.0 .0.as_mut_ptr().cast(),
                    MODULUS_REPR_LOW.as_ptr().cast(),
                );

                let _ = bigint_op_delegation_with_carry_bit::<ADD_OP_BIT_IDX>(
                    a.0 .0.as_mut_ptr().cast::<[u32; 8]>().add(1).cast(),
                    MODULUS_REPR_HIGH.as_ptr().cast(),
                    carry != 0,
                );
            }
        }
    }
    #[inline(always)]
    fn double_in_place(a: &mut F) {
        let tmp = a.0;
        __add_with_carry(&mut a.0, &tmp);
        __subtract_modulus(a);
    }
    /// Sets `a = -a`.
    #[inline(always)]
    fn neg_in_place(a: &mut F) {
        unsafe {
            let equal_low = bigint_op_delegation::<EQ_OP_BIT_IDX>(
                a.0 .0.as_mut_ptr().cast(),
                ZERO_REPR_0.as_ptr().cast(),
            );
            let equal_high = bigint_op_delegation::<EQ_OP_BIT_IDX>(
                a.0 .0.as_mut_ptr().cast::<[u32; 8]>().add(1).cast(),
                ZERO_REPR_0.as_ptr().cast(),
            );

            if equal_low == 0 || equal_high == 0 {
                let borrow = bigint_op_delegation::<SUB_AND_NEGATE_OP_BIT_IDX>(
                    a.0 .0.as_mut_ptr().cast(),
                    MODULUS_REPR_LOW.as_ptr().cast(),
                );
                let _ = bigint_op_delegation_with_carry_bit::<SUB_AND_NEGATE_OP_BIT_IDX>(
                    a.0 .0.as_mut_ptr().cast::<[u32; 8]>().add(1).cast(),
                    MODULUS_REPR_HIGH.as_ptr().cast(),
                    borrow != 0,
                );
            }
        }
    }
    #[inline(always)]
    fn mul_assign(a: &mut F, b: &F) {
        __mul_assign_impl(&mut a.0, &b.0);
        __subtract_modulus(a);
    }

    #[inline(always)]
    fn square_in_place(a: &mut F) {
        let tmp = a.0;
        __mul_assign_impl(&mut a.0, &tmp);
        __subtract_modulus(a);
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

#[inline(always)]
fn __subtract_modulus(a: &mut F) {
    unsafe {
        let dst_ptr = a.0 .0.as_mut_ptr().cast();

        let borrow =
            bigint_op_delegation::<SUB_OP_BIT_IDX>(dst_ptr, MODULUS_REPR_LOW.as_ptr().cast());
        LOW_WORD_SCRATCH.assume_init_mut()[0] = borrow;
        let dst_high_ptr = dst_ptr.add(8);
        // propagate borrow and sub high words
        let borrow = bigint_op_delegation_with_carry_bit::<SUB_OP_BIT_IDX>(
            dst_high_ptr,
            MODULUS_REPR_HIGH.as_ptr().cast(),
            borrow != 0,
        );

        if borrow != 0 {
            // adding a modulus back is faster than copying everything
            let carry =
                bigint_op_delegation::<ADD_OP_BIT_IDX>(dst_ptr, MODULUS_REPR_LOW.as_ptr().cast());
            let _ = bigint_op_delegation_with_carry_bit::<ADD_OP_BIT_IDX>(
                dst_high_ptr,
                MODULUS_REPR_HIGH.as_ptr().cast(),
                carry != 0,
            );
        }
    }
}

#[inline(always)]
fn __add_with_carry(a: &mut B, b: &B) -> bool {
    // there is no carry as we use canonical representation and have spare bits,
    // so we just add using precompile. Main problem is carry propagation across words,
    // and for now we use 3 calls for it

    unsafe {
        let b0: *const [u32; 8] = b.0.as_ptr().cast();
        let b0 = copy_if_needed(b0);
        let dst_ptr = a.0.as_mut_ptr().cast();
        let carry = bigint_op_delegation::<ADD_OP_BIT_IDX>(dst_ptr, b0.cast());
        let dst_high_ptr = dst_ptr.add(8);
        let b1: *const [u32; 8] = b.0.as_ptr().cast::<[u32; 8]>().add(1);
        let b1 = copy_if_needed(b1);
        // propagate carry and add high words of source
        let _ = bigint_op_delegation_with_carry_bit::<ADD_OP_BIT_IDX>(
            dst_high_ptr,
            b1.cast(),
            carry != 0,
        );
    }

    false
}

#[inline(always)]
fn __sub_with_borrow(a: &mut B, b: &B) -> bool {
    // same logic as above for add, but we can have borrow
    unsafe {
        let b0: *const [u32; 8] = b.0.as_ptr().cast();
        let b0 = copy_if_needed(b0);
        let dst_ptr = a.0.as_mut_ptr().cast();
        let borrow = bigint_op_delegation::<SUB_OP_BIT_IDX>(dst_ptr, b0.cast());
        let dst_high_ptr = dst_ptr.add(8);
        let b1: *const [u32; 8] = b.0.as_ptr().cast::<[u32; 8]>().add(1);
        let b1 = copy_if_needed(b1);
        // propagate borrow and sub high words of source
        let borrow = bigint_op_delegation_with_carry_bit::<SUB_OP_BIT_IDX>(
            dst_high_ptr,
            b1.cast(),
            borrow != 0,
        );

        borrow != 0
    }
}

#[inline(always)]
unsafe fn read_low(a: &B) -> [u32; 8] {
    // bigints are always "better" aligned, so it's enough
    // so cast pointer and read it
    a.0.as_ptr().cast::<[u32; 8]>().read()
}

#[inline(always)]
unsafe fn read_high(a: &B) -> [u32; 8] {
    a.0.as_ptr().cast::<[u32; 8]>().add(1).read()
}

#[inline(always)]
fn __mul_assign_impl(a: &mut B, b: &B) {
    // This one is more involved than BN254 case as we do "long arithmetics",
    // but with 256-bit words
    unsafe {
        // let mut r = [0u64; 2usize];

        let (r0, r1) = {
            let b0: *const [u32; 8] = b.0.as_ptr().cast();
            let b0 = copy_if_needed(b0);

            // let mut carry1 = 0u64;
            // r[0] = fa::mac(
            //     r[0],
            //     (a.0).0[0],
            //     (b.0).0[0usize],
            //     &mut carry1,
            // );

            // r0 and carry are 0 at this point

            let mut r0 = read_low(&*a);
            let mut carry_1 = r0;
            let _ = bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(r0.as_mut_ptr().cast(), b0.cast());
            let _ =
                bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(carry_1.as_mut_ptr().cast(), b0.cast());

            // let k = r[0].wrapping_mul(Self::INV);
            let mut reduction_k = MONT_REDUCTION_CONSTANT;
            let _ = bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(
                reduction_k.as_mut_ptr(),
                r0.as_ptr().cast(),
            );

            // let mut carry2 = 0u64;
            // fa::mac_discard(
            //     r[0],
            //     k,
            //     MODULUS_LOW,
            //     &mut carry2,
            // );
            let mut carry_2_low = MODULUS_CONSTANT_0;
            let _ = bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(
                carry_2_low.as_mut_ptr().cast(),
                reduction_k.as_ptr().cast(),
            );
            let of = bigint_op_delegation::<ADD_OP_BIT_IDX>(
                carry_2_low.as_mut_ptr().cast(),
                r0.as_ptr().cast(),
            );
            let mut carry_2 = MODULUS_CONSTANT_0;
            let _ = bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(
                carry_2.as_mut_ptr().cast(),
                reduction_k.as_ptr().cast(),
            );
            if of != 0 {
                let _ = bigint_op_delegation::<ADD_OP_BIT_IDX>(
                    carry_2.as_mut_ptr().cast(),
                    ONE_REPR.as_ptr().cast(),
                );
            }
            debug_assert_eq!(carry_2_low, [0u32; 8]);

            // r[1usize] = fa::mac_with_carry(
            //     r[1usize],
            //     (a.0).0[1usize],
            //     (b.0).0[0usize],
            //     &mut carry1,
            // );
            let mut r1 = read_high(&*a);
            let mut new_carry_1 = r1;
            // r1 is zero at this point, but not carry
            let _ = bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(r1.as_mut_ptr().cast(), b0.cast());
            let of = bigint_op_delegation::<ADD_OP_BIT_IDX>(
                r1.as_mut_ptr().cast(),
                carry_1.as_ptr().cast(),
            );
            let _ = bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(
                new_carry_1.as_mut_ptr().cast(),
                b0.cast(),
            );
            if of != 0 {
                let _ = bigint_op_delegation::<ADD_OP_BIT_IDX>(
                    new_carry_1.as_mut_ptr().cast(),
                    ONE_REPR.as_ptr().cast(),
                );
            }
            let carry_1 = new_carry_1;

            // now r0 and r1 are non-zero

            // r[0usize] = fa::mac_with_carry(
            //     r[1usize],
            //     k,
            //     MODULUS_HIGH,
            //     &mut carry2,
            // );
            let mut new_carry_2_low = MODULUS_CONSTANT_1;
            let _ = bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(
                new_carry_2_low.as_mut_ptr().cast(),
                reduction_k.as_ptr().cast(),
            );
            let of0 = bigint_op_delegation::<ADD_OP_BIT_IDX>(
                new_carry_2_low.as_mut_ptr().cast(),
                r1.as_ptr().cast(),
            );
            let of1 = bigint_op_delegation::<ADD_OP_BIT_IDX>(
                new_carry_2_low.as_mut_ptr().cast(),
                carry_2.as_ptr().cast(),
            );
            let mut new_carry_2 = MODULUS_CONSTANT_1;
            let _ = bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(
                new_carry_2.as_mut_ptr().cast(),
                reduction_k.as_ptr().cast(),
            );
            if of0 + of1 != 0 {
                LOW_WORD_SCRATCH.assume_init_mut()[0] = of0 + of1;
                let _ = bigint_op_delegation::<ADD_OP_BIT_IDX>(
                    new_carry_2.as_mut_ptr().cast(),
                    LOW_WORD_SCRATCH.as_ptr().cast(),
                );
            }
            let r0 = new_carry_2_low;
            let carry_2 = new_carry_2;

            // r[2usize - 1] = carry1 + carry2;
            let mut r1 = carry_1;
            let _ = bigint_op_delegation::<ADD_OP_BIT_IDX>(
                r1.as_mut_ptr().cast(),
                carry_2.as_ptr().cast(),
            );

            debug_assert!({
                let mut all_zeroes = true;
                for i in 4..8 {
                    all_zeroes &= r1[i] == 0;
                }

                all_zeroes
            });

            (r0, r1)
        };

        // now we use higher part of `b`

        let b1: *const [u32; 8] = b.0.as_ptr().cast::<[u32; 8]>().add(1);
        let b1 = copy_if_needed(b1);

        // let mut carry1 = 0u64;
        // r[0] = fa::mac(
        //     r[0],
        //     (a.0).0[0],
        //     (b.0).0[1usize],
        //     &mut carry1,
        // );
        let mut new_r0 = read_low(&*a);
        let mut carry_1 = new_r0;
        let _ = bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(new_r0.as_mut_ptr().cast(), b1.cast());
        let of =
            bigint_op_delegation::<ADD_OP_BIT_IDX>(new_r0.as_mut_ptr().cast(), r0.as_ptr().cast());
        let _ = bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(carry_1.as_mut_ptr().cast(), b1.cast());
        if of != 0 {
            let _ = bigint_op_delegation::<ADD_OP_BIT_IDX>(
                carry_1.as_mut_ptr().cast(),
                ONE_REPR.as_ptr().cast(),
            );
        }
        let r0 = new_r0;

        // let k = r[0].wrapping_mul(Self::INV);
        let mut reduction_k = MONT_REDUCTION_CONSTANT;
        let _ = bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(
            reduction_k.as_mut_ptr(),
            r0.as_ptr().cast(),
        );

        // let mut carry2 = 0u64;
        // fa::mac_discard(
        //     r[0],
        //     k,
        //     MODULUS_LOW,
        //     &mut carry2,
        // );
        let mut carry_2_low = MODULUS_CONSTANT_0;
        let _ = bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(
            carry_2_low.as_mut_ptr().cast(),
            reduction_k.as_ptr().cast(),
        );
        let of = bigint_op_delegation::<ADD_OP_BIT_IDX>(
            carry_2_low.as_mut_ptr().cast(),
            r0.as_ptr().cast(),
        );
        let mut carry_2 = MODULUS_CONSTANT_0;
        let _ = bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(
            carry_2.as_mut_ptr().cast(),
            reduction_k.as_ptr().cast(),
        );
        if of != 0 {
            let _ = bigint_op_delegation::<ADD_OP_BIT_IDX>(
                carry_2.as_mut_ptr().cast(),
                ONE_REPR.as_ptr().cast(),
            );
        }
        debug_assert_eq!(carry_2_low, [0u32; 8]);

        // r[1usize] = fa::mac_with_carry(
        //     r[1usize],
        //     (a.0).0[1usize],
        //     (b.0).0[1usize],
        //     &mut carry1,
        // );

        // r1 will NOT become part of the result, but carry will

        let mut new_r1 = read_high(&*a);
        let a_high_ptr = a.0.as_mut_ptr().cast::<[u32; 8]>().add(1);
        let _ = bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(new_r1.as_mut_ptr().cast(), b1.cast());
        let of0 = bigint_op_delegation::<ADD_OP_BIT_IDX>(
            new_r1.as_mut_ptr().cast(),
            carry_1.as_ptr().cast(),
        );
        let of1 =
            bigint_op_delegation::<ADD_OP_BIT_IDX>(new_r1.as_mut_ptr().cast(), r1.as_ptr().cast());
        let _ = bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(a_high_ptr.cast(), b1.cast());
        if of0 + of1 != 0 {
            LOW_WORD_SCRATCH.assume_init_mut()[0] = of0 + of1;
            let _ = bigint_op_delegation::<ADD_OP_BIT_IDX>(
                a_high_ptr.cast(),
                LOW_WORD_SCRATCH.as_ptr().cast(),
            );
        }
        // new carry_1 is in `a` now
        let r1 = new_r1;

        // r[0usize] = fa::mac_with_carry(
        //     r[1usize],
        //     k,
        //     MODULUS_HIGH,
        //     &mut carry2,
        // );

        // NOTE: r0 will become the result, so we use `a`
        let a_low_ptr = a.0.as_mut_ptr().cast::<[u32; 8]>();
        a_low_ptr.write(MODULUS_CONSTANT_1);

        let _ = bigint_op_delegation::<MUL_LOW_OP_BIT_IDX>(
            a_low_ptr.cast(),
            reduction_k.as_ptr().cast(),
        );
        let of0 = bigint_op_delegation::<ADD_OP_BIT_IDX>(a_low_ptr.cast(), r1.as_ptr().cast());
        let of1 = bigint_op_delegation::<ADD_OP_BIT_IDX>(a_low_ptr.cast(), carry_2.as_ptr().cast());
        let mut new_carry_2 = MODULUS_CONSTANT_1;
        let _ = bigint_op_delegation::<MUL_HIGH_OP_BIT_IDX>(
            new_carry_2.as_mut_ptr().cast(),
            reduction_k.as_ptr().cast(),
        );
        if of0 + of1 != 0 {
            LOW_WORD_SCRATCH.assume_init_mut()[0] = of0 + of1;
            let _ = bigint_op_delegation::<ADD_OP_BIT_IDX>(
                new_carry_2.as_mut_ptr().cast(),
                LOW_WORD_SCRATCH.as_ptr().cast(),
            );
        }
        let carry_2 = new_carry_2;

        // we again use high part of `a` as r1, and we already have carry1 there

        // r[2usize - 1] = carry1 + carry2;
        let _ = bigint_op_delegation::<ADD_OP_BIT_IDX>(a_high_ptr.cast(), carry_2.as_ptr().cast());

        debug_assert!({
            let mut all_zeroes = true;
            for i in 6..8 {
                all_zeroes &= a.0[i] == 0;
            }

            all_zeroes
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ark_ff::{Field, One, UniformRand, Zero};

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

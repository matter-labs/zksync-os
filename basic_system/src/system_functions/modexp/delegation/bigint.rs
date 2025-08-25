// Representation of big integers using primitives that are friendly for our delegations
extern crate alloc;

use super::u256::*;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::fmt::Debug;
use core::mem::MaybeUninit;
use crypto::{bigint_op_delegation_raw, bigint_op_delegation_with_carry_bit_raw, BigIntOps};
use zk_ee::system_io_oracle::Arithmetics;
use zk_ee::system_io_oracle::{ArithmeticsParam, IOOracle};

// There is a small choice to make - either we do exponentiation walking as via LE or BE exponent.
// If we do LE, then we square the base, and multiply accumulator by it
// If we do BE, then we square the accumulator, and then multiply it by base

// We have backing capacity (that we do not want to shrink),
// and actual counter in how many words we want to use
pub(crate) struct BigintRepr<A: Allocator + Clone> {
    pub(crate) backing: Vec<DelegatedU256, A>,
    pub(crate) digits: usize,
}

impl<A: Allocator + Clone> Debug for BigintRepr<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x")?;
        for digit in self.u64_digits_ref().iter().rev() {
            write!(f, "{:016x}", digit)?;
        }

        Ok(())
    }
}

impl<A: Allocator + Clone> BigintRepr<A> {
    pub(crate) fn with_capacity_in(capacity: usize, allocator: A) -> Self {
        let backing = Vec::with_capacity_in(capacity, allocator);

        Self { backing, digits: 0 }
    }

    pub(crate) fn duplicate_with_capacity(&self, capacity: usize, allocator: A) -> Self {
        unsafe {
            let mut backing = Vec::with_capacity_in(capacity, allocator);
            for (dst, src) in backing.spare_capacity_mut()[..self.digits_ref().len()]
                .iter_mut()
                .zip(self.digits_ref().iter())
            {
                write_into_ptr_unchecked(dst.as_mut_ptr(), src);
            }
            backing.set_len(self.digits_ref().len());

            Self {
                backing,
                digits: self.digits,
            }
        }
    }

    pub(crate) fn digits_ref(&self) -> &[DelegatedU256] {
        &self.backing[..self.digits]
    }

    pub(crate) fn digits_mut(&mut self) -> &mut [DelegatedU256] {
        &mut self.backing[..self.digits]
    }

    pub(crate) fn u64_digits_ref(&self) -> &[u64] {
        unsafe { core::slice::from_raw_parts(self.backing.as_ptr().cast(), self.digits * 4) }
    }

    pub(crate) fn clear_as_capacity_mut(&mut self) -> &mut [MaybeUninit<DelegatedU256>] {
        self.backing.clear();
        self.backing.spare_capacity_mut()
    }

    pub(crate) unsafe fn set_num_digits(&mut self, digits: usize) {
        self.backing.set_len(digits);
        self.digits = digits;
    }

    pub(crate) fn capacity(&self) -> usize {
        self.backing.capacity()
    }

    pub(crate) fn from_big_endian_with_double_capacity(bytes: &[u8], allocator: A) -> Self {
        if bytes.is_empty() {
            let backing = Vec::new_in(allocator);
            return Self { backing, digits: 0 };
        }
        let (remainder, digits_bytes) = bytes.as_rchunks::<32>();
        let mut capacity = digits_bytes.len();
        if remainder.is_empty() == false {
            capacity += 1;
        }
        let max_digits = capacity;
        capacity *= 2;

        Self::from_big_endian(remainder, digits_bytes, max_digits, capacity, allocator)
    }

    fn from_big_endian(
        remainder: &[u8],
        digits_bytes: &[[u8; 32]],
        max_digits: usize,
        capacity: usize,
        allocator: A,
    ) -> Self {
        let mut backing = Vec::with_capacity_in(capacity, allocator);
        for (dst, digit) in backing.spare_capacity_mut()[..digits_bytes.len()]
            .iter_mut()
            .zip(digits_bytes.iter().rev())
        {
            unsafe {
                DelegatedU256::from_be_bytes_in_place(digit, dst);
            }
        }
        if remainder.is_empty() == false {
            let dst = &mut backing.spare_capacity_mut()[digits_bytes.len()];
            let mut buffer = [0u8; 32];
            buffer[(32 - remainder.len())..].copy_from_slice(remainder);
            unsafe {
                DelegatedU256::from_be_bytes_in_place(&buffer, dst);
            }
        }
        unsafe {
            backing.set_len(max_digits);
        }

        let mut meaningful_digits = max_digits;
        for digit in backing.iter().rev() {
            if digit.is_zero() {
                meaningful_digits -= 1;
            } else {
                break;
            }
        }
        backing.truncate(meaningful_digits);

        Self {
            backing,
            digits: meaningful_digits,
        }
    }

    pub(crate) fn from_big_endian_with_double_capacity_or_min_capacity(
        bytes: &[u8],
        min_capacity: usize,
        allocator: A,
    ) -> Self {
        if bytes.is_empty() {
            let backing = Vec::new_in(allocator);
            return Self { backing, digits: 0 };
        }
        let (remainder, digits_bytes) = bytes.as_rchunks::<32>();
        let mut capacity = digits_bytes.len();
        if remainder.is_empty() == false {
            capacity += 1;
        }
        let max_digits = capacity;
        capacity *= 2;
        capacity = core::cmp::max(min_capacity, capacity);

        Self::from_big_endian(remainder, digits_bytes, max_digits, capacity, allocator)
    }

    pub(crate) fn modpow(
        self,
        exp: &[u8],
        modulus: Self,
        advisor: &mut impl ModexpAdvisor,
        allocator: A,
    ) -> Self {
        assert!(modulus.digits > 0);

        // We need some buffers, that will be used through the modular exponentiation,
        // and can be larger backing capacity than necessary, but we will only use the scratch space up to aprioiri known
        // bound. Modulus is assumed pristine

        // Initial reduction - we want to have a representation of self, such that
        // multiplications below are self-consistent. We do not even need to double-check strict
        // reduction as otherwise checks in exponentiation loop wouldn't pass anyway, so we just need to make
        // sure that number of digits is small enough

        let capacity_for_scratched_in_reduction =
            core::cmp::max(modulus.digits * 2, modulus.digits + self.digits);

        let mut scratch_0 =
            Self::with_capacity_in(capacity_for_scratched_in_reduction, allocator.clone());
        let mut scratch_1 =
            Self::with_capacity_in(capacity_for_scratched_in_reduction, allocator.clone());
        let mut scratch_2 =
            Self::with_capacity_in(capacity_for_scratched_in_reduction, allocator.clone());
        let mut digit_scratch_0 = DelegatedU256::zero();
        let mut digit_scratch_1 = DelegatedU256::zero();
        let mut digit_scratch_2 = DelegatedU256::zero();
        let mut digit_carry_propagation_scratch = DelegatedU256::zero();

        let mut current = self;

        // we will be a little conservative here, and also will handle the case of trivial exponent == 1,
        // but base > modulus
        if current.digits >= modulus.digits {
            (current, (scratch_0, scratch_1, scratch_2)) = Self::reduce_initially(
                current,
                &modulus,
                scratch_0,
                scratch_1,
                scratch_2,
                &mut digit_scratch_0,
                &mut digit_scratch_1,
                &mut digit_scratch_2,
                &mut digit_carry_propagation_scratch,
                advisor,
            );
        }
        assert!(current.digits <= modulus.digits);

        let base = current.duplicate_with_capacity(current.digits, allocator.clone());

        let mut scratch_3 = Self::with_capacity_in(modulus.digits * 2, allocator.clone());

        // we will go BE case to quickly strip leading zeroes
        let mut first_found = false;
        // Exp is BE, so do not need to reverse iterator
        'outer: for &byte in exp.iter() {
            // But here we should go from MSB
            for i in (0..8).rev() {
                if current.digits == 0 {
                    // in case if modulus is composite, we can get accumulator
                    // to be 0, and then we can exit the loop early
                    break 'outer;
                }
                let bit = byte & (1 << i) > 0;
                if first_found {
                    (current, (scratch_0, scratch_1, scratch_2, scratch_3)) = Self::square_step(
                        current,
                        &modulus,
                        scratch_0,
                        scratch_1,
                        scratch_2,
                        scratch_3,
                        &mut digit_scratch_0,
                        &mut digit_scratch_1,
                        &mut digit_scratch_2,
                        &mut digit_carry_propagation_scratch,
                        advisor,
                    );
                    if bit {
                        (current, (scratch_0, scratch_1, scratch_2, scratch_3)) = Self::mul_step(
                            current,
                            &base,
                            &modulus,
                            scratch_0,
                            scratch_1,
                            scratch_2,
                            scratch_3,
                            &mut digit_scratch_0,
                            &mut digit_scratch_1,
                            &mut digit_scratch_2,
                            &mut digit_carry_propagation_scratch,
                            advisor,
                        );
                    }
                } else {
                    if bit {
                        first_found = true;
                    }
                }
            }
        }

        if first_found {
            // at the very end we assert full reduction
            current.assert_fully_reduced(modulus);

            current
        } else {
            // anything in 0s power is 1
            let mut result = Vec::with_capacity_in(1, allocator);
            result.push(DelegatedU256::ONE);

            Self {
                backing: result,
                digits: 1,
            }
        }
    }

    // We assume everything coarsely reduced, so sizes of quotient and remainder can not have more digits
    #[inline(always)]
    fn reduce_initially(
        current: Self,
        modulus: &Self,
        mut scratch_0: Self,
        mut scratch_1: Self,
        mut scratch_2: Self,
        digit_scratch_0: &mut DelegatedU256,
        digit_scratch_1: &mut DelegatedU256,
        digit_scratch_2: &mut DelegatedU256,
        digit_carry_propagation_scratch: &mut DelegatedU256,
        advisor: &mut impl ModexpAdvisor,
    ) -> (Self, (Self, Self, Self)) {
        advisor.get_reduction_op_advise(&current, modulus, &mut scratch_0, &mut scratch_1);
        // now we should enforce everything backwards
        assert!(scratch_1.digits <= modulus.digits);

        assert!(scratch_2.capacity() >= modulus.digits + current.digits);

        // here we will use baseline FMA and scratches
        unsafe {
            Self::fma(
                &mut scratch_2,
                &scratch_0,
                &modulus,
                Some(&scratch_1),
                digit_scratch_0,
                digit_scratch_1,
                digit_scratch_2,
                digit_carry_propagation_scratch,
                scratch_0.digits + modulus.digits,
            );
        }

        // assert equality
        Self::assert_eq(&current, &scratch_2);

        // we always return remainder,
        // and the rest becomes scratches pool

        (scratch_1, (current, scratch_0, scratch_2))
    }

    // We assume everything coarsely reduced, so sizes of quotient and remainder can not have more digits
    #[inline(always)]
    fn mul_step(
        current: Self,
        other: &Self,
        modulus: &Self,
        mut scratch_0: Self,
        mut scratch_1: Self,
        mut scratch_2: Self,
        mut scratch_3: Self,
        digit_scratch_0: &mut DelegatedU256,
        digit_scratch_1: &mut DelegatedU256,
        digit_scratch_2: &mut DelegatedU256,
        digit_carry_propagation_scratch: &mut DelegatedU256,
        advisor: &mut impl ModexpAdvisor,
    ) -> (Self, (Self, Self, Self, Self)) {
        assert!(current.digits > 0); // case if it is 0 is handled by outer loop
        debug_assert_eq!(other.digits, modulus.digits); // we multiply accumulator by base, and base if fully reduced
        assert!(scratch_0.capacity() >= modulus.digits + 1);
        assert!(scratch_1.capacity() >= modulus.digits);
        assert!(scratch_2.capacity() >= modulus.digits * 2);
        assert!(scratch_3.capacity() >= modulus.digits * 2);

        // here we will use baseline FMA and scratches
        unsafe {
            Self::fma(
                &mut scratch_2,
                &current,
                &other,
                None,
                digit_scratch_0,
                digit_scratch_1,
                digit_scratch_2,
                digit_carry_propagation_scratch,
                current.digits + other.digits,
            );
            advisor.get_reduction_op_advise(&scratch_2, modulus, &mut scratch_0, &mut scratch_1);
            // now we should enforce everything backwards
            assert!(scratch_0.digits <= scratch_2.digits + 1 - modulus.digits);
            assert!(scratch_1.digits <= modulus.digits);

            Self::fma(
                &mut scratch_3,
                &scratch_0,
                &modulus,
                Some(&scratch_1),
                digit_scratch_0,
                digit_scratch_1,
                digit_scratch_2,
                digit_carry_propagation_scratch,
                scratch_2.digits,
            );
        }

        // assert equality
        Self::assert_eq(&scratch_2, &scratch_3);

        // we always return remainder,
        // and the rest becomes scratches pool

        (scratch_1, (current, scratch_0, scratch_2, scratch_3))
    }

    // We assume everything coarsely reduced, so sizes of quotient and remainder can not have more digits
    #[inline(always)]
    fn square_step(
        a: Self,
        modulus: &Self,
        mut scratch_0: Self,
        mut scratch_1: Self,
        mut scratch_2: Self,
        mut scratch_3: Self,
        digit_scratch_0: &mut DelegatedU256,
        digit_scratch_1: &mut DelegatedU256,
        digit_scratch_2: &mut DelegatedU256,
        digit_carry_propagation_scratch: &mut DelegatedU256,
        advisor: &mut impl ModexpAdvisor,
    ) -> (Self, (Self, Self, Self, Self)) {
        assert!(a.digits > 0); // case if it is 0 is handled by outer loop
        assert!(scratch_0.capacity() >= modulus.digits + 1);
        assert!(scratch_1.capacity() >= modulus.digits);
        assert!(scratch_2.capacity() >= modulus.digits * 2);
        assert!(scratch_3.capacity() >= modulus.digits * 2);

        // here we will use baseline FMA and scratches
        unsafe {
            Self::fma(
                &mut scratch_2,
                &a,
                &a,
                None,
                digit_scratch_0,
                digit_scratch_1,
                digit_scratch_2,
                digit_carry_propagation_scratch,
                a.digits * 2,
            );
            advisor.get_reduction_op_advise(&scratch_2, modulus, &mut scratch_0, &mut scratch_1);
            // now we should enforce everything backwards
            assert!(scratch_0.digits <= scratch_2.digits + 1 - modulus.digits);
            assert!(scratch_1.digits <= modulus.digits);

            Self::fma(
                &mut scratch_3,
                &scratch_0,
                &modulus,
                Some(&scratch_1),
                digit_scratch_0,
                digit_scratch_1,
                digit_scratch_2,
                digit_carry_propagation_scratch,
                scratch_2.digits,
            );
        }

        // assert equality
        Self::assert_eq(&scratch_2, &scratch_3);

        // we always return remainder,
        // and the rest becomes scratches pool

        (scratch_1, (a, scratch_0, scratch_2, scratch_3))
    }

    fn assert_eq(a: &Self, b: &Self) {
        let meaningful_digits_floor = core::cmp::min(a.digits, b.digits);
        for (a_digit, b_digit) in a.digits_ref().iter().zip(b.digits_ref().iter()) {
            assert!(a_digit.eq(b_digit));
        }
        for input in [a, b] {
            if input.digits > meaningful_digits_floor {
                for el in input.digits_ref()[meaningful_digits_floor..].iter() {
                    assert!(el.is_zero());
                }
            }
        }
    }

    fn assert_fully_reduced(&self, mut modulus: Self) {
        assert!(modulus.digits >= self.digits);
        if self.digits < modulus.digits {
            return;
        }

        // we need to perform long subtraction self - modulus always produces borrow,
        // but we do not want to kill self, so we will do inverse
        let mut borrow = 0;
        for (modulus_digit, self_digit) in modulus
            .digits_mut()
            .iter_mut()
            .zip(self.digits_ref().iter())
        {
            borrow = unsafe {
                bigint_op_delegation_with_carry_bit_raw(
                    (modulus_digit as *mut DelegatedU256).cast(),
                    (self_digit as *const DelegatedU256).cast(),
                    borrow > 0,
                    BigIntOps::SubAndNegate,
                )
            };
        }

        assert!(borrow > 0);
    }

    unsafe fn fma(
        dst_scratch: &mut Self,
        a: &Self,
        b: &Self,
        c: Option<&Self>,
        scratch_0: &mut DelegatedU256, // these three are just scratch space, we must write to them
        scratch_1: &mut DelegatedU256, // before trying to read
        scratch_2: &mut DelegatedU256,
        carry_propagation_scratch: &mut DelegatedU256, // this one has top limbs to be 0s
        max_product_digits: usize,
    ) {
        debug_assert_eq!(carry_propagation_scratch.as_limbs_mut()[1], 0);
        debug_assert_eq!(carry_propagation_scratch.as_limbs_mut()[2], 0);
        debug_assert_eq!(carry_propagation_scratch.as_limbs_mut()[3], 0);

        let dst_scratch_capacity = dst_scratch.clear_as_capacity_mut();
        assert!(dst_scratch_capacity.len() >= max_product_digits);
        if max_product_digits == 0 {
            if let Some(c) = c {
                assert_eq!(c.digits, 0);
            }
            dst_scratch.set_num_digits(0);
            return;
        }

        // schoolbook

        let mut next_to_init_digit = 0;
        if let Some(c) = c {
            // first write down c
            for c_digit_idx in 0..c.digits {
                write_into_ptr_unchecked(
                    dst_scratch_capacity[c_digit_idx].as_mut_ptr(),
                    c.backing.get_unchecked(c_digit_idx),
                );
            }
            next_to_init_digit = c.digits;
        }
        // we will pre-cast it to pointers for easier live, as we will rotate them
        let scratch_low = scratch_0 as *mut DelegatedU256;
        let mut scratch_high = scratch_1 as *mut DelegatedU256;
        let mut carry_scratch = scratch_2 as *mut DelegatedU256;

        for b_digit_idx in 0..b.digits {
            let b_digit = b.backing.get_unchecked(b_digit_idx) as *const DelegatedU256;
            for a_digit_idx in 0..a.digits {
                let a_digit = a.backing.get_unchecked(a_digit_idx);
                let dst_digit = a_digit_idx + b_digit_idx;

                assert!(next_to_init_digit >= dst_digit);

                if dst_digit == next_to_init_digit {
                    // scratch is uninit, so we consider it as 0 and can materialize low result directly there
                    // for double-width a * b

                    // scratch low and high are written if we were in the cycle at least once
                    write_into_ptr_unchecked(
                        dst_scratch_capacity[dst_digit].as_mut_ptr().cast(),
                        a_digit,
                    );
                    write_into_ptr_unchecked(scratch_high, a_digit);
                    let _ = bigint_op_delegation_raw(
                        dst_scratch_capacity[dst_digit].as_mut_ptr().cast(),
                        b_digit.cast(),
                        BigIntOps::MulLow,
                    );
                    let _ = bigint_op_delegation_raw(
                        scratch_high.cast(),
                        b_digit.cast(),
                        BigIntOps::MulHigh,
                    );
                    next_to_init_digit = dst_digit + 1;
                    if a_digit_idx > 0 {
                        // also add carry that we propagate while walking over "a" digits
                        let of = bigint_op_delegation_raw(
                            dst_scratch_capacity[dst_digit].as_mut_ptr().cast(),
                            carry_scratch.cast(),
                            BigIntOps::Add,
                        );

                        if of > 0 {
                            // and put this carry into high
                            carry_propagation_scratch.as_limbs_mut()[0] = of as u64;
                            // no carry is possible here
                            let _ = bigint_op_delegation_raw(
                                scratch_high.cast(),
                                (carry_propagation_scratch as *const DelegatedU256).cast(),
                                BigIntOps::Add,
                            );
                        }
                    }

                    // and renumerate - high is our new carry propagation
                    let t = carry_scratch;
                    carry_scratch = scratch_high;
                    scratch_high = t;
                } else {
                    // double-width a * b

                    // scratch low and high are written if we were in the cycle at least once
                    write_into_ptr_unchecked(scratch_low, a_digit);
                    write_into_ptr_unchecked(scratch_high, a_digit);
                    let _ = bigint_op_delegation_raw(
                        scratch_low.cast(),
                        b_digit.cast(),
                        BigIntOps::MulLow,
                    );
                    let _ = bigint_op_delegation_raw(
                        scratch_high.cast(),
                        b_digit.cast(),
                        BigIntOps::MulHigh,
                    );

                    // then we will add something from accumulator - it'll also write directly into destination
                    let of_0 = bigint_op_delegation_raw(
                        dst_scratch_capacity[dst_digit].as_mut_ptr().cast(),
                        scratch_low.cast(),
                        BigIntOps::Add,
                    );
                    let of_1 = if a_digit_idx > 0 {
                        // also add carry that we propagate while walking over "a" digits
                        bigint_op_delegation_raw(
                            dst_scratch_capacity[dst_digit].as_mut_ptr().cast(),
                            carry_scratch.cast(),
                            BigIntOps::Add,
                        )
                    } else {
                        0u32
                    };
                    // and put this carry into high
                    if of_0 + of_1 > 0 {
                        carry_propagation_scratch.as_limbs_mut()[0] = (of_0 + of_1) as u64;
                        // no carry is possible here
                        let _ = bigint_op_delegation_raw(
                            scratch_high.cast(),
                            (carry_propagation_scratch as *const DelegatedU256).cast(),
                            BigIntOps::Add,
                        );
                    }

                    // and renumerate
                    let t = carry_scratch;
                    carry_scratch = scratch_high;
                    scratch_high = t;
                }
            }
            if a.digits > 0 {
                // make final carry write - if can also initialize
                let dst_digit = a.digits + b_digit_idx;
                if dst_digit >= max_product_digits {
                    // abort propagation - we apriori expect that in well-formed case
                    // those digits can not exist
                } else {
                    assert!(next_to_init_digit >= dst_digit);
                    if dst_digit == next_to_init_digit {
                        let _ = bigint_op_delegation_raw(
                            dst_scratch_capacity[dst_digit].as_mut_ptr().cast(),
                            carry_scratch.cast(),
                            BigIntOps::MemCpy,
                        );
                        next_to_init_digit = dst_digit + 1;
                    } else {
                        let of = bigint_op_delegation_raw(
                            dst_scratch_capacity[dst_digit].as_mut_ptr().cast(),
                            carry_scratch.cast(),
                            BigIntOps::Add,
                        );
                        assert_eq!(of, 0);
                    }
                }
            }
        }

        assert!(next_to_init_digit <= max_product_digits);
        dst_scratch.set_num_digits(next_to_init_digit);
    }

    pub fn to_big_endian<B: Allocator>(&self, allocator: B) -> Vec<u8, B> {
        let mut result = Vec::with_capacity_in(self.digits * 32, allocator);
        for digit in self.digits_ref().iter().rev() {
            if digit.is_zero() {
                continue;
            }
            let be_bytes = digit.to_be_bytes();
            result.extend(be_bytes);
        }

        result
    }
}

pub(crate) trait ModexpAdvisor {
    // get advice for let (q,r) = div_rem(a, m)
    fn get_reduction_op_advise<A: Allocator + Clone>(
        &mut self,
        a: &BigintRepr<A>,
        m: &BigintRepr<A>,
        quotient_dst: &mut BigintRepr<A>,
        remainder_dst: &mut BigintRepr<A>,
    );
}

#[cfg(test)]
pub(crate) mod naive_advisor {
    use std::alloc::Global;

    use super::*;
    use num_bigint::BigUint;

    fn write_bigint(src: BigUint, dst: &mut BigintRepr<impl Allocator + Clone>) {
        unsafe {
            let mut src = src.iter_u64_digits();
            let dst_capacity = dst.clear_as_capacity_mut();
            let mut digits = 0;
            for dst in dst_capacity.iter_mut() {
                let dst: *mut u64 = dst.as_mut_ptr().cast::<[u64; 4]>().cast();
                let mut exhausted = false;
                for i in 0..4 {
                    if let Some(digit) = src.next() {
                        dst.add(i).write(digit);
                        if i == 0 {
                            digits += 1;
                        }
                    } else {
                        dst.add(i).write(0);
                        exhausted = true;
                    }
                }
                if exhausted {
                    break;
                }
            }
            assert!(src.next().is_none());
            dst.set_num_digits(digits);
        }
    }

    pub(crate) struct NaiveAdvisor;

    impl ModexpAdvisor for NaiveAdvisor {
        fn get_reduction_op_advise<A: Allocator + Clone>(
            &mut self,
            a: &BigintRepr<A>,
            m: &BigintRepr<A>,
            quotient_dst: &mut BigintRepr<A>,
            remainder_dst: &mut BigintRepr<A>,
        ) {
            let a = a.to_big_endian(Global);
            let a = BigUint::from_bytes_be(&a);

            assert!(m.digits > 0);
            let m = m.to_big_endian(Global);
            let m = BigUint::from_bytes_be(&m);

            use num_traits::ops::euclid::Euclid;
            let (q, r) = a.div_rem_euclid(&m);

            write_bigint(q, quotient_dst);
            write_bigint(r, remainder_dst);
        }
    }
}

pub(crate) struct OracleAdvisor<'a, O: IOOracle> {
    pub(crate) inner: &'a mut O,
}

fn write_bigint(
    it: &mut impl ExactSizeIterator<Item = usize>,
    mut to_consume: usize,
    dst: &mut BigintRepr<impl Allocator + Clone>,
) {
    const {
        assert!(core::mem::size_of::<usize>() == core::mem::size_of::<u32>());
    }
    unsafe {
        let num_digits = to_consume.next_multiple_of(8) / 8;
        let dst_capacity = dst.clear_as_capacity_mut();
        for dst in dst_capacity[..num_digits].iter_mut() {
            let dst: *mut u32 = dst.as_mut_ptr().cast::<[u32; 8]>().cast();
            for i in 0..8 {
                if to_consume > 0 {
                    to_consume -= 1;
                    let digit = it.next().unwrap();
                    dst.add(i).write(digit as u32);
                } else {
                    dst.add(i).write(0);
                }
            }
        }
        assert_eq!(to_consume, 0);
        dst.set_num_digits(num_digits);
    }
}

impl<'a, O: IOOracle> ModexpAdvisor for OracleAdvisor<'a, O> {
    fn get_reduction_op_advise<A: Allocator + Clone>(
        &mut self,
        a: &BigintRepr<A>,
        m: &BigintRepr<A>,
        quotient_dst: &mut BigintRepr<A>,
        remainder_dst: &mut BigintRepr<A>,
    ) {
        let arg: ArithmeticsParam = {
            let a_len = a.digits;
            let a_ptr = a.backing.as_ptr();

            let modulus_len = m.digits;
            let modulus_ptr = m.backing.as_ptr();

            assert!(modulus_len > 0);

            let arg = ArithmeticsParam {
                op: 0,
                a_ptr: a_ptr.addr() as u32,
                a_len: a_len as u32,
                b_ptr: 0,
                b_len: 0,
                modulus_ptr: modulus_ptr.addr() as u32,
                modulus_len: modulus_len as u32,
            };

            arg
        };

        // We assume that oracle's response is well-formed lengths-wise, and we will check value-wise separately
        let mut it = self
            .inner
            .create_oracle_access_iterator::<Arithmetics>(
                (&arg as *const ArithmeticsParam).addr() as u32
            )
            .unwrap();

        let q_len = it.next().expect("quotient length");
        let r_len = it.next().expect("remainder length");

        let max_quotient_digits = a.digits + 1 - m.digits;
        let max_remainder_digits = m.digits;

        const {
            assert!(core::mem::size_of::<usize>() == core::mem::size_of::<u32>());
        }

        // check that hint is "sane" in upper bound

        assert!(q_len.next_multiple_of(8) / 8 <= max_quotient_digits);
        assert!(r_len.next_multiple_of(8) / 8 <= max_remainder_digits);

        write_bigint(&mut it, q_len, quotient_dst);
        write_bigint(&mut it, r_len, remainder_dst);

        assert!(it.next().is_none());
    }
}

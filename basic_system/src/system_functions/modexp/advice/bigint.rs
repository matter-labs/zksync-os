// Representation of big integers using primitives that are friendly for our delegations
extern crate alloc;

#[cfg(all(
    not(target_arch = "riscv32"),
    not(all(target_pointer_width = "64", target_endian = "little"))
))]
compile_error!("host-side modexp advice handling requires a 64-bit little-endian host target");

use super::super::MODEXP_ADVICE_QUERY_ID;
use super::u256::*;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::fmt::Debug;
use core::mem::MaybeUninit;
use crypto::{bigint_op_delegation_raw, bigint_op_delegation_with_carry_bit_raw, BigIntOps};
use ruint::aliases::U256;
use zk_ee::oracle::word_layout::WordLayout;
use zk_ee::oracle::IOOracle;

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
            write!(f, "{digit:016x}")?;
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

    /// # Safety
    ///
    /// `digits` must not exceed `self.backing.capacity()`, and the first
    /// `digits` elements of `self.backing` must have been fully initialized.
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
            let backing = Vec::with_capacity_in(min_capacity, allocator);
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
        advisor: &mut impl ModexpAdvisor<Alloc = A>,
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
        // quotient
        let mut scratch_0 =
            Self::with_capacity_in(capacity_for_scratched_in_reduction, allocator.clone());
        // remainder
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

        debug_assert!(base.digits <= modulus.digits);

        // we will go BE case to quickly strip leading zeroes
        let mut first_found = false;
        // Exp is BE, so do not need to reverse iterator
        'outer: for &byte in exp.iter() {
            // But here we should go from MSB
            for i in (0..8).rev() {
                debug_assert!(base.digits <= modulus.digits);
                let bit = byte & (1 << i) > 0;
                if first_found {
                    if current.digits == 0 {
                        // in case if modulus is composite, we can get accumulator
                        // to be 0, and then we can exit the loop early. And it's not 0^0 case
                        break 'outer;
                    }
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
                        if current.digits == 0 {
                            break 'outer;
                        }
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
                } else if bit {
                    first_found = true;
                }
            }
        }

        debug_assert!(base.digits <= modulus.digits);

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
        advisor: &mut impl ModexpAdvisor<Alloc = A>,
    ) -> (Self, (Self, Self, Self)) {
        advisor.get_reduction_op_advice(&current, modulus, &mut scratch_0, &mut scratch_1);
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
        advisor: &mut impl ModexpAdvisor<Alloc = A>,
    ) -> (Self, (Self, Self, Self, Self)) {
        assert!(current.digits > 0); // case if it is 0 is handled by outer loop
        debug_assert!(other.digits <= modulus.digits); // we multiply accumulator by base, and base if fully reduced
        assert!(scratch_0.capacity() > modulus.digits);
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
            advisor.get_reduction_op_advice(&scratch_2, modulus, &mut scratch_0, &mut scratch_1);
            // now we should enforce everything backwards
            let max_q = if scratch_2.digits < modulus.digits {
                0
            } else if scratch_2.digits == modulus.digits {
                1
            } else {
                scratch_2.digits + 1 - modulus.digits
            };
            assert!(scratch_0.digits <= max_q);

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
        advisor: &mut impl ModexpAdvisor<Alloc = A>,
    ) -> (Self, (Self, Self, Self, Self)) {
        assert!(a.digits > 0); // case if it is 0 is handled by outer loop
        assert!(scratch_0.capacity() > modulus.digits);
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
            advisor.get_reduction_op_advice(&scratch_2, modulus, &mut scratch_0, &mut scratch_1);
            // now we should enforce everything backwards
            let max_q = if scratch_2.digits < modulus.digits {
                0
            } else if scratch_2.digits == modulus.digits {
                1
            } else {
                scratch_2.digits + 1 - modulus.digits
            };
            assert!(scratch_0.digits <= max_q);
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
            #[allow(clippy::needless_range_loop)]
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
                    core::mem::swap(&mut carry_scratch, &mut scratch_high);
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
                    core::mem::swap(&mut carry_scratch, &mut scratch_high);
                }
            }
            if a.digits > 0 {
                // make final carry write - if can also initialize
                let dst_digit = a.digits + b_digit_idx;
                if dst_digit >= max_product_digits {
                    // abort propagation - we apriori expect that in well-formed case
                    // those digits can not exist
                    debug_assert!((*carry_scratch).is_zero());
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
                        let mut of = bigint_op_delegation_raw(
                            dst_scratch_capacity[dst_digit].as_mut_ptr().cast(),
                            carry_scratch.cast(),
                            BigIntOps::Add,
                        );

                        let mut current_digit = dst_digit;
                        while of > 0 {
                            current_digit += 1;
                            debug_assert!(current_digit < max_product_digits);
                            carry_propagation_scratch.as_limbs_mut()[0] = of as u64;

                            if current_digit == next_to_init_digit {
                                let _ = bigint_op_delegation_raw(
                                    dst_scratch_capacity[current_digit].as_mut_ptr().cast(),
                                    (carry_propagation_scratch as *const DelegatedU256).cast(),
                                    BigIntOps::MemCpy,
                                );
                                next_to_init_digit = current_digit + 1;
                                break;
                            } else {
                                of = bigint_op_delegation_raw(
                                    dst_scratch_capacity[current_digit].as_mut_ptr().cast(),
                                    (carry_propagation_scratch as *const DelegatedU256).cast(),
                                    BigIntOps::Add,
                                );
                            }
                        }
                    }
                }
            }
        }

        assert!(next_to_init_digit <= max_product_digits);
        dst_scratch.set_num_digits(next_to_init_digit);
    }

    pub fn to_big_endian<B: Allocator>(&self, allocator: B) -> Vec<u8, B> {
        let mut result = Vec::with_capacity_in(self.digits * 32, allocator);
        let mut found_non_zero = false;
        for digit in self.digits_ref().iter().rev() {
            if digit.is_zero() == false {
                found_non_zero = true;
            }

            // Skip zeroed suffix if any
            if found_non_zero {
                let be_bytes = digit.to_be_bytes();
                result.extend(be_bytes);
            }
        }

        result
    }
}

pub(crate) trait ModexpAdvisor {
    type Alloc: Allocator + Clone;

    fn get_reduction_op_advice(
        &mut self,
        a: &BigintRepr<Self::Alloc>,
        m: &BigintRepr<Self::Alloc>,
        quotient_dst: &mut BigintRepr<Self::Alloc>,
        remainder_dst: &mut BigintRepr<Self::Alloc>,
    );
}

#[cfg(any(test, feature = "testing"))]
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
        type Alloc = Global;

        fn get_reduction_op_advice(
            &mut self,
            a: &BigintRepr<Global>,
            m: &BigintRepr<Global>,
            quotient_dst: &mut BigintRepr<Global>,
            remainder_dst: &mut BigintRepr<Global>,
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

pub(crate) struct OracleAdvisor<'a, O: IOOracle, A: Allocator + Clone> {
    inner: &'a mut O,
    response_buf: ModexpResponse<A>,
}

impl<'a, O: IOOracle, A: Allocator + Clone> OracleAdvisor<'a, O, A> {
    pub(crate) fn new(oracle: &'a mut O, allocator: A) -> Self {
        Self {
            inner: oracle,
            response_buf: ModexpResponse::new(allocator),
        }
    }
}

/// Number of u32 words per DelegatedU256 digit (256 bits / 32 bits = 8).
const BIGINT_DIGIT_U32_SIZE: usize = U256::BYTES / core::mem::size_of::<u32>();
/// Number of u64 limbs per DelegatedU256 digit (256 bits / 64 bits = 4).
const BIGINT_DIGIT_U64_SIZE: usize = 4;

/// WordLayout for BigintRepr: wire format is Vec<DelegatedU256>
/// (u32 digit count + DelegatedU256 elements, each 8 u32 words).
/// read_words_into writes DelegatedU256 values directly into the backing.
impl<A: Allocator + Clone> WordLayout for BigintRepr<A> {
    const WORD_COUNT: Option<usize> = None;

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        (self.digits as u32).write_words(w);
        for digit in self.digits_ref() {
            digit.write_words(w);
        }
    }

    fn read_words(_r: &mut impl FnMut() -> u32) -> Self {
        panic!("BigintRepr::read_words requires a pre-allocated instance; use read_words_into")
    }

    fn read_words_into(&mut self, r: &mut impl FnMut() -> u32) {
        let num_digits = u32::read_words(r) as usize;
        self.backing.clear();
        self.backing.reserve(num_digits);
        for _ in 0..num_digits {
            self.backing.push(DelegatedU256::read_words(r));
        }
        self.digits = num_digits;
    }
}

/// Modexp oracle response holding BigintReprs directly. Wire-compatible with
/// the processor's Vec<u64> encoding (both use u32 length + u64 elements).
/// Modexp oracle response. Holds BigintReprs for zero-copy read_words_into.
/// Use `from_u64_slices` on the host to construct from division results.
pub struct ModexpResponse<A: Allocator + Clone = alloc::alloc::Global> {
    quotient: BigintRepr<A>,
    remainder: BigintRepr<A>,
}

impl<A: Allocator + Clone> ModexpResponse<A> {
    pub(crate) fn new(allocator: A) -> Self {
        Self {
            quotient: BigintRepr::with_capacity_in(0, allocator.clone()),
            remainder: BigintRepr::with_capacity_in(0, allocator),
        }
    }

    pub fn quotient_u64s(&self) -> &[u64] {
        self.quotient.u64_digits_ref()
    }

    pub fn remainder_u64s(&self) -> &[u64] {
        self.remainder.u64_digits_ref()
    }
}

impl ModexpResponse<alloc::alloc::Global> {
    /// Construct from u64 slices (host/processor side, after ruint division).
    pub fn from_u64_slices(quotient: &[u64], remainder: &[u64]) -> Self {
        let mut q = BigintRepr::with_capacity_in(
            quotient.len().div_ceil(BIGINT_DIGIT_U64_SIZE),
            alloc::alloc::Global,
        );
        write_bigint_from_u64_digits(quotient, &mut q);
        let mut r = BigintRepr::with_capacity_in(
            remainder.len().div_ceil(BIGINT_DIGIT_U64_SIZE),
            alloc::alloc::Global,
        );
        write_bigint_from_u64_digits(remainder, &mut r);
        Self {
            quotient: q,
            remainder: r,
        }
    }
}

fn write_bigint_from_u64_digits(digits: &[u64], dst: &mut BigintRepr<impl Allocator + Clone>) {
    unsafe {
        let num_digits =
            digits.len().next_multiple_of(BIGINT_DIGIT_U64_SIZE) / BIGINT_DIGIT_U64_SIZE;
        let dst_capacity = dst.clear_as_capacity_mut();
        let mut src_idx = 0;
        for dst_slot in dst_capacity[..num_digits].iter_mut() {
            let dst_ptr: *mut u64 = dst_slot
                .as_mut_ptr()
                .cast::<[u64; BIGINT_DIGIT_U64_SIZE]>()
                .cast();
            for i in 0..BIGINT_DIGIT_U64_SIZE {
                if src_idx < digits.len() {
                    dst_ptr.add(i).write(digits[src_idx]);
                    src_idx += 1;
                } else {
                    dst_ptr.add(i).write(0);
                }
            }
        }
        dst.set_num_digits(num_digits);
    }
}

impl<A: Allocator + Clone> WordLayout for ModexpResponse<A> {
    const WORD_COUNT: Option<usize> = None;

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        self.quotient.write_words(w);
        self.remainder.write_words(w);
    }

    fn read_words(_r: &mut impl FnMut() -> u32) -> Self {
        panic!("ModexpResponse::read_words requires pre-allocated instance; use read_words_into")
    }

    fn read_words_into(&mut self, r: &mut impl FnMut() -> u32) {
        self.quotient.read_words_into(r);
        self.remainder.read_words_into(r);
    }
}

fn write_bigint_from_u32_words(words: &[u32], dst: &mut BigintRepr<impl Allocator + Clone>) {
    // NOTE: even if oracle overstates the number of digits (so - iterator length), it is not important
    // as long as caller checks that number of digits is within bounds of soundness
    // Safety: we only write initialized u32 chunks into spare capacity and
    // then set the number of initialized big-int digits accordingly.
    let word_count = words.len();
    unsafe {
        let num_digits = word_count.next_multiple_of(BIGINT_DIGIT_U32_SIZE) / BIGINT_DIGIT_U32_SIZE;
        let dst_capacity = dst.clear_as_capacity_mut();
        let mut consumed = 0;
        for dst in dst_capacity[..num_digits].iter_mut() {
            let dst: *mut u32 = dst
                .as_mut_ptr()
                .cast::<[u32; BIGINT_DIGIT_U32_SIZE]>()
                .cast();
            for i in 0..BIGINT_DIGIT_U32_SIZE {
                if consumed < word_count {
                    dst.add(i).write(words[consumed]);
                    consumed += 1;
                } else {
                    dst.add(i).write(0);
                }
            }
        }
        assert_eq!(consumed, word_count);
        dst.set_num_digits(num_digits);
    }
}

/// Serializes bigint digits as a Vec of u32 words for use as oracle query input.
fn bigint_to_u32_words<A: Allocator + Clone>(repr: &BigintRepr<A>) -> Vec<u32> {
    let mut words = Vec::with_capacity(repr.digits * BIGINT_DIGIT_U32_SIZE);
    for digit in repr.digits_ref() {
        let ptr = (digit as *const DelegatedU256).cast::<u32>();
        for i in 0..BIGINT_DIGIT_U32_SIZE {
            words.push(unsafe { ptr.add(i).read() });
        }
    }
    words
}

/// Oracle input for modexp reduction: operation code, dividend words, modulus words.
/// Uses field-by-field WordLayout since it contains Vec (dynamic type).
#[derive(Clone, Debug, WordLayout)]
struct ModexpReductionInput {
    op: u32,
    a_words: Vec<u32>,
    modulus_words: Vec<u32>,
}

impl<'a, O: IOOracle, A: Allocator + Clone> ModexpAdvisor for OracleAdvisor<'a, O, A> {
    type Alloc = A;

    fn get_reduction_op_advice(
        &mut self,
        a: &BigintRepr<A>,
        m: &BigintRepr<A>,
        quotient_dst: &mut BigintRepr<A>,
        remainder_dst: &mut BigintRepr<A>,
    ) {
        assert!(m.digits > 0);

        let input = ModexpReductionInput {
            op: 0,
            a_words: bigint_to_u32_words(a),
            modulus_words: bigint_to_u32_words(m),
        };

        // query_into reads directly into BigintRepr backing via read_words_into.
        // No intermediate Vec — u64 limbs go straight into DelegatedU256 slots.
        // Allocations in response_buf are reused across the ~384 calls.
        self.inner
            .query_into(MODEXP_ADVICE_QUERY_ID, &input, &mut self.response_buf)
            .unwrap();

        let max_quotient_digits = if a.digits < m.digits {
            0
        } else if a.digits == m.digits {
            1
        } else {
            a.digits + 1 - m.digits
        };

        let max_remainder_digits = m.digits;

        assert!(self.response_buf.quotient.digits <= max_quotient_digits);
        assert!(self.response_buf.remainder.digits <= max_remainder_digits);

        // Swap: destinations get fresh data, response_buf gets old scratch
        // data (capacity preserved for next call's read_words_into). Zero copy.
        core::mem::swap(&mut self.response_buf.quotient, quotient_dst);
        core::mem::swap(&mut self.response_buf.remainder, remainder_dst);
    }
}

#[cfg(test)]
mod tests {
    use std::alloc::Global;

    use super::*;
    use zk_ee::oracle::word_layout::WordLayout;
    use zk_ee::system::errors::internal::InternalError;

    /// A test oracle that returns a ModexpResponse with oversized quotient/remainder.
    struct OversizedResponseOracle {
        quotient: Vec<u64>,
        remainder: Vec<u64>,
    }

    impl IOOracle for OversizedResponseOracle {
        fn query<I: WordLayout, O: WordLayout>(
            &mut self,
            query_type: u32,
            _input: &I,
        ) -> Result<O, InternalError> {
            assert_eq!(query_type, MODEXP_ADVICE_QUERY_ID);
            let response = ModexpResponse::from_u64_slices(&self.quotient, &self.remainder);
            let mut words = Vec::new();
            response.write_words(&mut |w| words.push(w));
            let mut idx = 0;
            Ok(O::read_words(&mut || {
                let w = words.get(idx).copied().unwrap_or(0);
                idx += 1;
                w
            }))
        }
    }

    fn assert_oversized_response_panics(q_limbs: Vec<u64>, r_limbs: Vec<u64>) {
        super::super::u256::init();

        let dividend = BigintRepr::from_big_endian_with_double_capacity(&[0xA5; 96], Global);
        let modulus = BigintRepr::from_big_endian_with_double_capacity(&[0x5A; 64], Global);
        let mut quotient = BigintRepr::with_capacity_in(4, Global);
        let mut remainder = BigintRepr::with_capacity_in(4, Global);
        let mut oracle = OversizedResponseOracle {
            quotient: q_limbs,
            remainder: r_limbs,
        };
        let mut advisor = OracleAdvisor {
            inner: &mut oracle,
            response_buf: Default::default(),
        };

        advisor.get_reduction_op_advice(&dividend, &modulus, &mut quotient, &mut remainder);
    }

    #[test]
    #[should_panic]
    fn oracle_advisor_rejects_oversized_quotient() {
        assert_oversized_response_panics(vec![1u64; 20], vec![0u64; 4]);
    }

    #[test]
    #[should_panic]
    fn oracle_advisor_rejects_oversized_remainder() {
        assert_oversized_response_panics(vec![0u64; 4], vec![1u64; 20]);
    }
}

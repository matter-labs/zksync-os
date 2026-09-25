// Representation of big integers using primitives that are friendly for our delegations
extern crate alloc;

#[cfg(all(
    not(target_arch = "riscv32"),
    not(all(target_pointer_width = "64", target_endian = "little"))
))]
compile_error!("host-side modexp advice handling requires a 64-bit little-endian host target");

use super::super::MODEXP_ADVICE_QUERY_ID;
use super::exponent::{exponentiate, ModMul};
use super::u256::*;
use crate::system_functions::modexp::strip_leading_zeroes;
use crate::system_functions::modexp::ModExpAdviceParams64;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::fmt::Debug;
use core::mem::MaybeUninit;
use crypto::{bigint_op_delegation_raw, bigint_op_delegation_with_carry_bit_raw, BigIntOps};
use ruint::aliases::U256;
use u256::U256 as HintU256;
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
        advisor: &mut impl ModexpAdvisor,
        allocator: A,
    ) -> Self {
        assert!(modulus.digits > 0);
        let n = modulus.digits;
        let mut current = self;

        // The buffers of a multiplication step, sized for the exponent loop (products of two
        // reduced values) and for the initial reduction of an oversized base.
        let mut arithmetic = BigintModMul {
            modulus: &modulus,
            product: Self::with_capacity_in(2 * n, allocator.clone()),
            quotient: Self::with_capacity_in(
                core::cmp::max(n + 1, current.digits + 1 - n.min(current.digits)),
                allocator.clone(),
            ),
            remainder: Self::with_capacity_in(n, allocator.clone()),
            check: Self::with_capacity_in(core::cmp::max(2 * n, current.digits), allocator.clone()),
            scratch: ProductScratch::new(),
            advisor,
            allocator: allocator.clone(),
        };

        // Initial reduction, whenever the base is not shorter than the modulus (the cost model
        // mirrors this gate on digit counts): afterwards the value has at most `n` digits, and
        // every later remainder too. Strict reduction of the intermediate values is not
        // needed: an unreduced remainder is congruent and the final result is checked.
        if current.digits >= n {
            arithmetic.reduce_initially(&mut current);
        }
        assert!(current.digits <= n);

        // 1^exp mod m = 1 for any exp (m > 1 guaranteed by caller)
        if current.digits == 1 && current.backing[0].is_one() {
            return current;
        }

        let exp = strip_leading_zeroes(exp);
        if exp.is_empty() {
            // anything in 0s power is 1
            let mut result = Vec::with_capacity_in(1, allocator);
            result.push(DelegatedU256::ONE);
            return Self {
                backing: result,
                digits: 1,
            };
        }

        // 0^exp mod m = 0 for exp > 0
        if current.digits == 0 {
            return current;
        }

        let result = exponentiate(&mut arithmetic, &current, exp);
        drop(arithmetic);

        // at the very end we assert full reduction
        result.assert_fully_reduced(modulus);
        result
    }

    /// `self` = zero with `digits` (initialized) digits
    fn fill_zero(&mut self, digits: usize) {
        let capacity = self.clear_as_capacity_mut();
        assert!(capacity.len() >= digits);
        for slot in capacity[..digits].iter_mut() {
            // SAFETY: an aligned slot of the backing vector
            unsafe { DelegatedU256::write_zero_into_ptr(slot.as_mut_ptr()) };
        }
        // SAFETY: the first `digits` slots were just written
        unsafe { self.set_num_digits(digits) };
    }

    /// Drops the zero top digits
    fn trim(&mut self) {
        let mut digits = self.digits;
        while digits > 0 && self.backing[digits - 1].is_zero() {
            digits -= 1;
        }
        // SAFETY: shrinking within the initialized prefix
        unsafe { self.set_num_digits(digits) };
    }

    /// `digits[k..] += lo + hi * 2^256` (a 512-bit value at digit position `k`), with carry
    /// propagation; panics if the sum does not fit in `digits`
    ///
    /// # Safety
    /// `lo` and `hi` must point to aligned, initialized digits outside `digits`.
    unsafe fn add_wide_at(
        digits: &mut [DelegatedU256],
        k: usize,
        lo: *const DelegatedU256,
        hi: *const DelegatedU256,
    ) {
        let len = digits.len();
        assert!(k < len, "the product does not fit");
        let base = digits.as_mut_ptr();
        let mut carry =
            bigint_op_delegation_raw(base.add(k).cast(), lo.cast(), BigIntOps::Add) != 0;
        let mut j = k + 1;
        if j < len {
            carry = bigint_op_delegation_with_carry_bit_raw(
                base.add(j).cast(),
                hi.cast(),
                carry,
                BigIntOps::Add,
            ) != 0;
            j += 1;
        } else {
            assert!(!carry && (*hi).is_zero(), "the product does not fit");
        }
        while carry {
            assert!(j < len, "the product does not fit");
            carry = bigint_op_delegation_with_carry_bit_raw(
                base.add(j).cast(),
                DelegatedU256::zero_ptr().cast(),
                true,
                BigIntOps::Add,
            ) != 0;
            j += 1;
        }
    }

    /// `self = a * b` (schoolbook), with the zero top digits dropped
    fn mul_into(&mut self, a: &Self, b: &Self, scratch: &mut ProductScratch) {
        if a.digits == 0 || b.digits == 0 {
            self.fill_zero(0);
            return;
        }
        self.fill_zero(a.digits + b.digits);
        let out = self.digits_mut();
        for (j, b_digit) in b.digits_ref().iter().enumerate() {
            for (i, a_digit) in a.digits_ref().iter().enumerate() {
                // SAFETY: the scratch slots are distinct from `out`
                unsafe {
                    scratch.product(a_digit, b_digit);
                    Self::add_wide_at(out, i + j, &scratch.lo, &scratch.hi);
                }
            }
        }
        self.trim();
    }

    /// `self = a * a`: each cross product `a_i a_j` (`i < j`) is computed once and added
    /// twice, `n (n + 1) / 2` digit products instead of `n^2`
    fn square_into(&mut self, a: &Self, scratch: &mut ProductScratch) {
        if a.digits == 0 {
            self.fill_zero(0);
            return;
        }
        self.fill_zero(2 * a.digits);
        let out = self.digits_mut();
        let a_digits = a.digits_ref();
        for (i, a_i) in a_digits.iter().enumerate() {
            for (j, a_j) in a_digits.iter().enumerate().skip(i + 1) {
                // SAFETY: the scratch slots are distinct from `out`
                unsafe {
                    scratch.product(a_i, a_j);
                    Self::add_wide_at(out, i + j, &scratch.lo, &scratch.hi);
                    Self::add_wide_at(out, i + j, &scratch.lo, &scratch.hi);
                }
            }
        }
        for (i, a_i) in a_digits.iter().enumerate() {
            // SAFETY: as above
            unsafe {
                scratch.product(a_i, a_i);
                Self::add_wide_at(out, 2 * i, &scratch.lo, &scratch.hi);
            }
        }
        self.trim();
    }

    /// `self = q * m + r` in exactly `digits` digits; panics if the value does not fit, so a
    /// hint reconstructing a truncated value cannot pass the comparison with the product
    fn fma_into(
        &mut self,
        q: &Self,
        m: &Self,
        r: &Self,
        digits: usize,
        scratch: &mut ProductScratch,
    ) {
        assert!(r.digits <= digits, "the remainder hint is too long");
        self.fill_zero(digits);
        let out = self.digits_mut();
        for (slot, r_digit) in out.iter_mut().zip(r.digits_ref()) {
            // SAFETY: `r` is a distinct, initialized value
            unsafe { write_into_ptr_unchecked(slot, r_digit) };
        }
        for (j, m_digit) in m.digits_ref().iter().enumerate() {
            for (i, q_digit) in q.digits_ref().iter().enumerate() {
                // SAFETY: the scratch slots are distinct from `out`
                unsafe {
                    scratch.product(q_digit, m_digit);
                    Self::add_wide_at(out, i + j, &scratch.lo, &scratch.hi);
                }
            }
        }
    }

    /// The most digits a quotient of a `dividend_digits`-digit value by the modulus can have
    fn max_quotient_digits(dividend_digits: usize, modulus_digits: usize) -> usize {
        if dividend_digits < modulus_digits {
            0
        } else if dividend_digits == modulus_digits {
            1
        } else {
            dividend_digits + 1 - modulus_digits
        }
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

/// Two aligned slots holding the 512-bit product of two digits
struct ProductScratch {
    lo: DelegatedU256,
    hi: DelegatedU256,
}

impl ProductScratch {
    fn new() -> Self {
        Self {
            lo: DelegatedU256::zero(),
            hi: DelegatedU256::zero(),
        }
    }

    /// `(lo, hi) = a * b`
    ///
    /// # Safety
    /// `a` and `b` must be initialized digits outside ROM.
    #[inline(always)]
    unsafe fn product(&mut self, a: &DelegatedU256, b: &DelegatedU256) {
        write_into_ptr_unchecked(&mut self.lo, a);
        write_into_ptr_unchecked(&mut self.hi, a);
        let b = (b as *const DelegatedU256).cast();
        let _ = bigint_op_delegation_raw(
            (&mut self.lo as *mut DelegatedU256).cast(),
            b,
            BigIntOps::MulLow,
        );
        let _ = bigint_op_delegation_raw(
            (&mut self.hi as *mut DelegatedU256).cast(),
            b,
            BigIntOps::MulHigh,
        );
    }
}

/// Modular multiplication of multi-digit values: the product, an advised quotient and
/// remainder, and the check `q * m + r == product`. The remainder becomes the new value by
/// swapping buffers (every value buffer has capacity for `n` digits).
struct BigintModMul<'a, A: Allocator + Clone, Adv: ModexpAdvisor> {
    modulus: &'a BigintRepr<A>,
    product: BigintRepr<A>,
    quotient: BigintRepr<A>,
    remainder: BigintRepr<A>,
    check: BigintRepr<A>,
    scratch: ProductScratch,
    advisor: &'a mut Adv,
    allocator: A,
}

impl<A: Allocator + Clone, Adv: ModexpAdvisor> BigintModMul<'_, A, Adv> {
    /// `x = value mod m` for the `value` in `x` (at least as many digits as the modulus)
    fn reduce_initially(&mut self, x: &mut BigintRepr<A>) {
        assert!(x.digits >= self.modulus.digits);
        core::mem::swap(x, &mut self.product);
        self.reduce_product_into(x);
    }

    /// `x = product mod m`: the advised remainder, after the check of the advice
    fn reduce_product_into(&mut self, x: &mut BigintRepr<A>) {
        let n = self.modulus.digits;
        assert!(self.product.digits > 0);
        self.advisor.get_reduction_op_advice(
            &self.product,
            self.modulus,
            &mut self.quotient,
            &mut self.remainder,
        );
        assert!(
            self.quotient.digits <= BigintRepr::<A>::max_quotient_digits(self.product.digits, n)
        );
        assert!(self.remainder.digits <= n);
        self.check.fma_into(
            &self.quotient,
            self.modulus,
            &self.remainder,
            self.product.digits,
            &mut self.scratch,
        );
        BigintRepr::assert_eq(&self.product, &self.check);
        assert!(self.remainder.capacity() >= n && x.capacity() >= n);
        core::mem::swap(x, &mut self.remainder);
    }
}

impl<A: Allocator + Clone, Adv: ModexpAdvisor> ModMul for BigintModMul<'_, A, Adv> {
    type Elem = BigintRepr<A>;

    fn placeholder(&mut self) -> BigintRepr<A> {
        BigintRepr {
            backing: Vec::new_in(self.allocator.clone()),
            digits: 0,
        }
    }

    fn clone_elem(&mut self, x: &BigintRepr<A>) -> BigintRepr<A> {
        x.duplicate_with_capacity(self.modulus.digits, self.allocator.clone())
    }

    fn is_zero(&self, x: &BigintRepr<A>) -> bool {
        x.digits == 0
    }

    fn square_assign(&mut self, x: &mut BigintRepr<A>) {
        if x.digits == 0 {
            return;
        }
        self.product.square_into(x, &mut self.scratch);
        self.reduce_product_into(x);
    }

    fn mul_assign(&mut self, x: &mut BigintRepr<A>, y: &BigintRepr<A>) {
        if x.digits == 0 {
            return;
        }
        if y.digits == 0 {
            x.fill_zero(0);
            return;
        }
        self.product.mul_into(x, y, &mut self.scratch);
        self.reduce_product_into(x);
    }
}

pub(crate) trait ModexpAdvisor {
    /// The quotient and remainder of `a / m`
    fn get_reduction_op_advice<A: Allocator + Clone>(
        &mut self,
        a: &BigintRepr<A>,
        m: &BigintRepr<A>,
        quotient_dst: &mut BigintRepr<A>,
        remainder_dst: &mut BigintRepr<A>,
    );

    /// The quotient `(hi * 2^256 + lo) / modulus` as `(q_lo, q_hi)`, for the single-digit
    /// modulus path
    fn wide_quotient(
        &mut self,
        lo: &HintU256,
        hi: &HintU256,
        modulus: &HintU256,
    ) -> (HintU256, HintU256);
}

/// `(hi * 2^256 + lo) / modulus` computed natively, as `(q_lo, q_hi)`
#[cfg(any(test, feature = "testing"))]
pub(crate) fn naive_wide_quotient(
    lo: &HintU256,
    hi: &HintU256,
    modulus: &HintU256,
) -> (HintU256, HintU256) {
    let mut dividend = [0u64; 8];
    dividend[..4].copy_from_slice(lo.as_limbs());
    dividend[4..].copy_from_slice(hi.as_limbs());
    let mut divisor = *modulus.as_limbs();
    ruint::algorithms::div(&mut dividend, &mut divisor);
    let mut q_lo = [0u64; 4];
    let mut q_hi = [0u64; 4];
    q_lo.copy_from_slice(&dividend[..4]);
    q_hi.copy_from_slice(&dividend[4..]);
    (HintU256::from_limbs(q_lo), HintU256::from_limbs(q_hi))
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
        fn get_reduction_op_advice<A: Allocator + Clone>(
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

        fn wide_quotient(
            &mut self,
            lo: &HintU256,
            hi: &HintU256,
            modulus: &HintU256,
        ) -> (HintU256, HintU256) {
            naive_wide_quotient(lo, hi, modulus)
        }
    }
}
pub(crate) struct OracleAdvisor<'a, O: IOOracle> {
    pub(crate) inner: &'a mut O,
}

const BIGINT_DIGIT_USIZE_SIZE: usize = U256::BYTES / core::mem::size_of::<usize>();

/// Reads `to_consume` words of a hint into `dst`, whole digits at a time (the last digit
/// zero-padded)
fn write_bigint(
    it: &mut impl ExactSizeIterator<Item = usize>,
    to_consume: usize,
    dst: &mut BigintRepr<impl Allocator + Clone>,
) {
    let num_digits = to_consume.div_ceil(BIGINT_DIGIT_USIZE_SIZE);
    let dst_capacity = dst.clear_as_capacity_mut();
    assert!(dst_capacity.len() >= num_digits, "the hint is too long");
    let full_digits = to_consume / BIGINT_DIGIT_USIZE_SIZE;
    let tail_words = to_consume % BIGINT_DIGIT_USIZE_SIZE;
    let (full, partial) = dst_capacity[..num_digits].split_at_mut(full_digits);
    // SAFETY: every word of the first `num_digits` slots is written below
    unsafe {
        for digit in full.iter_mut() {
            let words = digit.as_mut_ptr().cast::<usize>();
            for i in 0..BIGINT_DIGIT_USIZE_SIZE {
                words.add(i).write(it.next().expect("hint word"));
            }
        }
        if let Some(digit) = partial.first_mut() {
            let words = digit.as_mut_ptr().cast::<usize>();
            for i in 0..tail_words {
                words.add(i).write(it.next().expect("hint word"));
            }
            for i in tail_words..BIGINT_DIGIT_USIZE_SIZE {
                words.add(i).write(0);
            }
        }
        dst.set_num_digits(num_digits);
    }
}

impl<'a, O: IOOracle> ModexpAdvisor for OracleAdvisor<'a, O> {
    fn get_reduction_op_advice<A: Allocator + Clone>(
        &mut self,
        a: &BigintRepr<A>,
        m: &BigintRepr<A>,
        quotient_dst: &mut BigintRepr<A>,
        remainder_dst: &mut BigintRepr<A>,
    ) {
        // We use different advice params depending on architecture
        // Both are mostly the same, main difference is the width of pointers
        #[cfg(target_pointer_width = "32")]
        let (mut it, q_len, r_len) = {
            use crate::system_functions::modexp::ModExpAdviceParams;
            let arg: ModExpAdviceParams = {
                let a_len = a.digits;
                let a_ptr = a.backing.as_ptr();

                let modulus_len = m.digits;
                let modulus_ptr = m.backing.as_ptr();

                assert!(modulus_len > 0);

                ModExpAdviceParams {
                    op: 0,
                    a_ptr: a_ptr.addr() as u32,
                    a_len: a_len as u32,
                    b_ptr: 0,
                    b_len: 0,
                    modulus_ptr: modulus_ptr.addr() as u32,
                    modulus_len: modulus_len as u32,
                }
            };
            // We assume that oracle's response is well-formed lengths-wise, and we will check value-wise separately
            let mut it = self
                .inner
                .raw_query(
                    MODEXP_ADVICE_QUERY_ID,
                    &((&arg as *const ModExpAdviceParams).addr() as u32),
                )
                .unwrap();
            let q_len = it.next().expect("quotient length");
            let r_len = it.next().expect("remainder length");
            (it, q_len, r_len)
        };

        #[cfg(target_pointer_width = "64")]
        let (mut it, q_len, r_len) = {
            let arg: ModExpAdviceParams64 = {
                let a_len = a.digits;
                let a_ptr = a.backing.as_ptr();

                let modulus_len = m.digits;
                let modulus_ptr = m.backing.as_ptr();

                assert!(modulus_len > 0);

                ModExpAdviceParams64 {
                    op: 0,
                    a_ptr: a_ptr.addr() as u64,
                    a_len: a_len as u64,
                    b_ptr: 0,
                    b_len: 0,
                    modulus_ptr: modulus_ptr.addr() as u64,
                    modulus_len: modulus_len as u64,
                }
            };
            // We assume that oracle's response is well-formed lengths-wise, and we will check value-wise separately
            let mut it = self
                .inner
                .raw_query(
                    MODEXP_ADVICE_QUERY_ID,
                    &((&arg as *const ModExpAdviceParams64).addr() as u64),
                )
                .unwrap();
            // Oracle provides lengths as u32, so in this case they are
            // packed into a single usize
            // Note lengths are in 32-bit words, so we have to divide
            // by 2 on 64-bit arch.
            let packed_lens = it.next().expect("packed lengths");
            let q_len = (packed_lens & 0xFFFF_FFFF) as usize;
            let r_len = (packed_lens >> 32) as usize;
            assert!(
                q_len.is_multiple_of(2) && r_len.is_multiple_of(2),
                "oracle returned an odd number of u32 words"
            );
            (it, q_len / 2, r_len / 2)
        };

        let max_quotient_digits = if a.digits < m.digits {
            0
        } else if a.digits == m.digits {
            1
        } else {
            a.digits + 1 - m.digits
        };

        let max_remainder_digits = m.digits;

        // check that hint is "sane" in upper bound

        assert!(
            q_len.next_multiple_of(BIGINT_DIGIT_USIZE_SIZE) / BIGINT_DIGIT_USIZE_SIZE
                <= max_quotient_digits
        );
        assert!(
            r_len.next_multiple_of(BIGINT_DIGIT_USIZE_SIZE) / BIGINT_DIGIT_USIZE_SIZE
                <= max_remainder_digits
        );

        write_bigint(&mut it, q_len, quotient_dst);
        write_bigint(&mut it, r_len, remainder_dst);

        assert!(it.next().is_none());
    }

    fn wide_quotient(
        &mut self,
        lo: &HintU256,
        hi: &HintU256,
        modulus: &HintU256,
    ) -> (HintU256, HintU256) {
        crate::system_functions::u256_advice::query_wide_div_rem_hint(lo, hi, modulus, self.inner)
    }
}

#[cfg(test)]
mod tests {
    use std::alloc::Global;

    use super::*;
    use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
    use zk_ee::system::errors::internal::InternalError;

    struct PackedLengthOracle {
        packed_lens: usize,
    }

    impl zk_ee::oracle::memory_io::MemoryOracle for PackedLengthOracle {}

    impl IOOracle for PackedLengthOracle {
        type RawIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            query_type: u32,
            _input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            assert_eq!(query_type, MODEXP_ADVICE_QUERY_ID);
            Ok(Box::new([self.packed_lens].into_iter()))
        }
    }

    fn assert_odd_word_count_panics(packed_lens: usize) {
        let dividend = BigintRepr::from_big_endian_with_double_capacity(&[0xA5; 96], Global);
        let modulus = BigintRepr::from_big_endian_with_double_capacity(&[0x5A; 64], Global);
        let mut quotient = BigintRepr::with_capacity_in(4, Global);
        let mut remainder = BigintRepr::with_capacity_in(4, Global);
        let mut oracle = PackedLengthOracle { packed_lens };
        let mut advisor = OracleAdvisor { inner: &mut oracle };

        advisor.get_reduction_op_advice(&dividend, &modulus, &mut quotient, &mut remainder);
    }

    #[test]
    #[should_panic(expected = "oracle returned an odd number of u32 words")]
    fn oracle_advisor_rejects_odd_quotient_word_count() {
        assert_odd_word_count_panics(3 | (4 << 32));
    }

    #[test]
    #[should_panic(expected = "oracle returned an odd number of u32 words")]
    fn oracle_advisor_rejects_odd_remainder_word_count() {
        assert_odd_word_count_panics(4 | (3 << 32));
    }
}

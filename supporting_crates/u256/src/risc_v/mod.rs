use core::ops::{
    AddAssign, BitAndAssign, BitOrAssign, BitXorAssign, ShlAssign, ShrAssign, SubAssign,
};
use delegated_u256::*;

// ---------------------------------------------------------------------------
// Oracle CSR helpers for non-deterministic division hints (RISC-V only)
// ---------------------------------------------------------------------------

/// Oracle query ID for U256 division hints.
/// Must match `zk_ee::oracle::query_ids::U256_DIV_REM_ADVICE_QUERY_ID`.
#[cfg(target_arch = "riscv32")]
const U256_DIV_REM_ADVICE_QUERY_ID: u32 = 0x4005_0030;

/// Oracle query ID for U256 mulmod hints.
/// Must match `zk_ee::oracle::query_ids::U256_MULMOD_ADVICE_QUERY_ID`.
#[cfg(target_arch = "riscv32")]
const U256_MULMOD_ADVICE_QUERY_ID: u32 = 0x4005_0031;

/// Write a word to the oracle CSR (address 0x7c0).
/// Mirrors `riscv_common::csr_write_word`.
#[cfg(target_arch = "riscv32")]
#[inline(always)]
fn oracle_csr_write(value: usize) {
    unsafe {
        core::arch::asm!(
            "csrrw x0, 0x7c0, {rd}",
            rd = in(reg) value,
            options(nomem, nostack, preserves_flags)
        )
    }
}

/// Read a word from the oracle CSR (address 0x7c0).
/// Mirrors `riscv_common::csr_read_word`.
#[cfg(target_arch = "riscv32")]
#[inline(always)]
fn oracle_csr_read() -> u32 {
    let output;
    unsafe {
        core::arch::asm!(
            "csrrw {rd}, 0x7c0, x0",
            rd = out(reg) output,
            options(nomem, nostack, preserves_flags)
        );
    }
    output
}

/// Read a U256 from the oracle CSR stream (4 limbs, each as two u32 reads).
#[cfg(target_arch = "riscv32")]
fn read_u256_from_csr() -> U256 {
    // SAFETY: `[u64; 4]` is a plain integer array with no drop glue and no
    // validity invariants beyond being initialized. The loop below writes every
    // element exactly once via `iter_mut()` before `from_limbs` reads them.
    let mut limbs: [u64; 4] = unsafe { core::mem::MaybeUninit::uninit().assume_init() };
    for limb in limbs.iter_mut() {
        let lo = oracle_csr_read() as u64;
        let hi = oracle_csr_read() as u64;
        *limb = lo | (hi << 32);
    }
    U256::from_limbs(limbs)
}

/// Query the oracle for `(quotient, remainder)` and verify the hint using delegated arithmetic.
///
/// The oracle response is untrusted. Verification ensures:
/// - `q * d + r == n` (widening mul + add + equality)
/// - No 256-bit overflow (`hi == 0`, no carry)
/// - `r < d` (remainder is fully reduced)
///
/// Together these uniquely determine `q` and `r` for given `(n, d)`.
#[cfg(target_arch = "riscv32")]
fn oracle_div_rem(dividend: &mut U256, divisor: &mut U256) {
    // ---- Send oracle query ----
    oracle_csr_write(U256_DIV_REM_ADVICE_QUERY_ID as usize);
    oracle_csr_write(2); // 2 u32 words to follow
    oracle_csr_write((dividend as *const U256).addr());
    oracle_csr_write((divisor as *const U256).addr());

    // Response: 4 q limbs + 4 r limbs = 16 u32 reads.
    let response_len = oracle_csr_read();
    assert_eq!(response_len, 16);

    let quotient = read_u256_from_csr();
    let remainder = read_u256_from_csr();

    // ---- Verify hint ----
    // Check: q * d + r == n (original dividend), with no 256-bit overflow.

    // widening_mul_assign_into(low, high, rhs) computes: low = low_256(low * rhs),
    // high = high_256(low * rhs). Both `low` and `high` must start as the same value
    // (the original multiplicand) because MUL_LOW overwrites `low` first.
    let mut check_lo = quotient.clone();
    let mut check_hi = quotient.clone();
    check_lo
        .0
        .widening_mul_assign_into(&mut check_hi.0, &divisor.0);

    // check_lo += r
    let carry = check_lo.0.overflowing_add_assign(&remainder.0);

    // No overflow: high part must be zero and no carry from addition
    assert!(!carry && check_hi.0.is_zero_mut());

    // q * d + r must equal the original dividend
    assert!(check_lo == *dividend);

    // Remainder must be strictly less than divisor (fully reduced).
    assert!(remainder < *divisor);

    // ---- Write results ----
    *dividend = quotient;
    *divisor = remainder;
}

/// Query the oracle for mulmod hint `(q, r)` such that `a * b == q * m + r` with `r < m`.
///
/// The oracle response is untrusted. Verification ensures:
/// - `q * m + r == a * b` (widening muls + adds + equality)
/// - No 512-bit overflow in `q * m + r`
/// - `r < m` (remainder is fully reduced)
///
/// Together these uniquely determine `r` for given `(a, b, m)`.
///
/// Panics if `modulus_or_result` is zero (caller must guard against this).
///
/// `q` is up to 512 bits (two U256 words: q_lo, q_hi).
#[cfg(target_arch = "riscv32")]
fn oracle_mulmod(a: &mut U256, b: &mut U256, modulus_or_result: &mut U256) {
    // ---- Send oracle query ----
    oracle_csr_write(U256_MULMOD_ADVICE_QUERY_ID as usize);
    oracle_csr_write(3); // 3 u32 words to follow
    oracle_csr_write((a as *const U256).addr());
    oracle_csr_write((b as *const U256).addr());
    oracle_csr_write((modulus_or_result as *const U256).addr());

    // Response: q_lo (4 limbs) + q_hi (4 limbs) + r (4 limbs) = 24 u32 reads.
    let response_len = oracle_csr_read();
    assert_eq!(response_len, 24);

    let q_lo = read_u256_from_csr();
    let q_hi = read_u256_from_csr();
    let remainder = read_u256_from_csr();

    // ---- Verify hint ----
    // Check: q*m + r == a*b, with no overflow beyond 512 bits.
    //
    // q = q_lo + q_hi * 2^256, so:
    // q*m = q_lo*m + (q_hi*m) << 256
    //
    // Let (p0_lo, p0_hi) = widening_mul(q_lo, m)
    // Let (p1_lo, p1_hi) = widening_mul(q_hi, m)
    //
    // Then q*m + r has:
    //   low 256:  p0_lo + r
    //   high 256: p0_hi + p1_lo + carry
    //   overflow: p1_hi + carry2 (must be zero)

    // (p0_lo, p0_hi) = widening_mul(q_lo, m)
    let mut p0_lo = q_lo.clone();
    let mut p0_hi = q_lo.clone();
    p0_lo
        .0
        .widening_mul_assign_into(&mut p0_hi.0, &modulus_or_result.0);

    // (p1_lo, p1_hi) = widening_mul(q_hi, m)
    let mut p1_lo = q_hi.clone();
    let mut p1_hi = q_hi.clone();
    p1_lo
        .0
        .widening_mul_assign_into(&mut p1_hi.0, &modulus_or_result.0);

    // p1_hi must be zero (result fits in 512 bits)
    assert!(p1_hi.0.is_zero_mut());

    // check_lo = p0_lo + r
    let c1 = p0_lo.0.overflowing_add_assign(&remainder.0);

    // check_hi = p0_hi + p1_lo + c1
    let c2a = p0_hi.0.overflowing_add_assign(&p1_lo.0);
    let c2b = p0_hi
        .0
        .overflowing_add_assign_with_carry(&U256::from_limbs([0, 0, 0, 0]).0, c1);
    assert!(!(c2a | c2b));

    // Compute (ab_lo, ab_hi) = widening_mul(a, b)
    let mut ab_lo = a.clone();
    let mut ab_hi = a.clone();
    ab_lo.0.widening_mul_assign_into(&mut ab_hi.0, &b.0);

    // q*m + r must equal a*b
    assert!(p0_lo == ab_lo);
    assert!(p0_hi == ab_hi);

    // Remainder must be strictly less than modulus
    assert!(remainder < *modulus_or_result);

    // ---- Write result ----
    *modulus_or_result = remainder;
}

// Even though we derive, internally we use delegation circuit for equality, ordering and cloning
// See DelegatedU256 implementations for details
#[derive(Clone, Hash, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct U256(DelegatedU256);

impl core::fmt::Display for U256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::LowerHex::fmt(self, f)
    }
}

impl core::fmt::LowerHex for U256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        <DelegatedU256 as core::fmt::LowerHex>::fmt(&self.0, f)
    }
}

impl core::default::Default for U256 {
    #[inline(always)]
    fn default() -> Self {
        Self::zero()
    }
}

impl U256 {
    pub const ZERO: Self = Self(DelegatedU256::ZERO);
    pub const ONE: Self = Self(DelegatedU256::ONE);

    pub const BYTES: usize = 32;

    pub const fn from_limbs(limbs: [u64; 4]) -> Self {
        Self(DelegatedU256::from_limbs(limbs))
    }

    /// # Safety
    /// `dst` must be 32 byte aligned and point to 32 bytes of accessible memory.
    pub unsafe fn write_into_ptr(dst: *mut Self, source: &Self) {
        delegated_u256::write_into_ptr(dst.cast(), &source.0);
    }

    /// # Safety
    /// `dst` must be 32 byte aligned and point to 32 bytes of accessible memory.
    pub unsafe fn write_into_ptr_unchecked(dst: *mut Self, source: &Self) {
        delegated_u256::write_into_ptr_unchecked(dst.cast(), &source.0);
    }

    /// # Safety
    /// `a` and `b` must be valid, properly aligned pointers to initialized `Self` values.
    ///
    /// On the delegated backend this is cheaper than a generic `mem::swap`, because it stays on
    /// the bigint memcopy path instead of forcing a raw 32-byte move sequence in RISC-V code.
    pub unsafe fn swap_in_place(a: *mut Self, b: *mut Self) {
        if core::ptr::eq(a, b) {
            return;
        }

        let mut tmp = core::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            Self::write_into_ptr_unchecked(tmp.as_mut_ptr(), &*a);
            Self::write_into_ptr_unchecked(a, &*b);
            Self::write_into_ptr_unchecked(b, tmp.assume_init_ref());
        }
    }

    pub fn clone_into(&self, dst: &mut Self) {
        unsafe { Self::write_into_ptr(dst as *mut _, self) };
    }

    pub unsafe fn clone_into_unchecked(&self, dst: &mut Self) {
        Self::write_into_ptr_unchecked(dst as *mut _, self);
    }

    #[inline(always)]
    pub fn zero() -> Self {
        Self(DelegatedU256::zero())
    }

    #[inline(always)]
    pub fn one() -> Self {
        Self(DelegatedU256::one())
    }

    pub fn bytereverse(&mut self) {
        self.0.bytereverse();
    }

    #[inline(always)]
    pub fn write_zero(into: &mut Self) {
        into.0.write_zero();
    }

    #[inline(always)]
    pub fn write_one(into: &mut Self) {
        into.0.write_one();
    }

    #[inline(always)]
    pub unsafe fn write_zero_into_ptr(into: *mut Self) {
        delegated_u256::write_zero_into_ptr(into.cast());
    }

    #[inline(always)]
    pub unsafe fn write_one_into_ptr(into: *mut Self) {
        delegated_u256::write_one_into_ptr(into.cast());
    }

    #[inline(always)]
    pub unsafe fn write_u64_into_ptr(into: *mut Self, value: u64) {
        delegated_u256::write_u64_into_ptr(into.cast(), value);
    }

    #[inline(always)]
    pub const fn as_limbs(&self) -> &[u64; 4] {
        self.0.as_limbs()
    }

    #[inline(always)]
    pub fn as_limbs_mut(&mut self) -> &mut [u64; 4] {
        self.0.as_limbs_mut()
    }

    #[inline(always)]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    #[inline(always)]
    pub fn is_one(&self) -> bool {
        self.0.is_one()
    }

    #[inline(always)]
    pub fn overflowing_add_assign(&mut self, rhs: &Self) -> bool {
        self.0.overflowing_add_assign(&rhs.0)
    }

    #[inline(always)]
    pub fn overflowing_add(mut self, rhs: Self) -> (Self, bool) {
        let carry = self.0.overflowing_add_assign(&rhs.0);
        (self, carry)
    }

    #[inline(always)]
    pub fn overflowing_add_assign_with_carry_propagation(
        &mut self,
        rhs: &Self,
        carry: bool,
    ) -> bool {
        self.0.overflowing_add_assign_with_carry(&rhs.0, carry)
    }

    #[inline(always)]
    pub fn overflowing_sub_assign(&mut self, rhs: &Self) -> bool {
        self.0.overflowing_sub_assign(&rhs.0)
    }

    #[inline(always)]
    pub fn overflowing_sub(mut self, rhs: Self) -> (Self, bool) {
        let borrow = self.0.overflowing_sub_assign(&rhs.0);
        (self, borrow)
    }

    #[inline(always)]
    pub fn overflowing_sub_assign_reversed(&mut self, rhs: &Self) -> bool {
        self.0.overflowing_sub_and_negate_assign(&rhs.0)
    }

    #[inline(always)]
    pub fn wrapping_mul_assign(&mut self, rhs: &Self) {
        self.0.mul_low_assign(&rhs.0);
    }

    #[inline(always)]
    pub fn high_mul_assign(&mut self, rhs: &Self) {
        self.0.mul_high_assign(&rhs.0);
    }

    #[inline(always)]
    pub fn widening_mul_assign(&mut self, rhs: &Self) -> Self {
        let result = self.0.widening_mul_assign(&rhs.0);
        Self(result)
    }

    #[inline(always)]
    pub fn widening_mul_assign_into(&mut self, high: &mut Self, rhs: &Self) {
        self.0.widening_mul_assign_into(&mut high.0, &rhs.0);
    }

    #[inline(always)]
    /// Panics if divisor is 0
    pub fn div_rem(dividend_or_quotient: &mut Self, divisor_or_remainder: &mut Self) {
        let is_zero = divisor_or_remainder.0.is_zero_mut();
        assert!(is_zero == false);

        #[cfg(target_arch = "riscv32")]
        {
            oracle_div_rem(dividend_or_quotient, divisor_or_remainder);
        }

        #[cfg(not(target_arch = "riscv32"))]
        {
            ruint::algorithms::div(
                dividend_or_quotient.as_limbs_mut(),
                divisor_or_remainder.as_limbs_mut(),
            );
        }
    }

    #[inline(always)]
    /// Panics if divisor is 0
    pub fn div_ceil(dividend_or_quotient: &mut Self, divisor: &Self) {
        let mut divisor_or_remainder = divisor.clone();
        Self::div_rem(dividend_or_quotient, &mut divisor_or_remainder);

        if !divisor_or_remainder.0.is_zero_mut() {
            let overflowed = dividend_or_quotient.overflowing_add_assign(&Self::one());
            assert!(overflowed == false); // Should not ever overflow
        }
    }

    #[inline(always)]
    pub fn not_mut(&mut self) {
        self.0.not_assign()
    }

    pub fn try_from_be_slice(input: &[u8]) -> Option<Self> {
        match input.try_into() {
            Ok(bytes) => Some(Self::from_be_bytes(bytes)),
            Err(_) => None,
        }
    }

    pub fn from_be_bytes(input: &[u8; 32]) -> Self {
        Self(DelegatedU256::from_be_bytes(input))
    }

    pub fn from_le_bytes(input: &[u8; 32]) -> Self {
        Self(DelegatedU256::from_le_bytes(input))
    }

    pub fn to_le_bytes(&self) -> [u8; 32] {
        self.0.to_le_bytes()
    }

    pub fn to_be_bytes(&self) -> [u8; 32] {
        self.0.to_be_bytes()
    }

    pub fn bit_len(&self) -> usize {
        self.0.bit_len()
    }

    pub fn leading_zeros(&self) -> usize {
        self.0.leading_zeros()
    }

    pub fn byte(&self, byte_idx: usize) -> u8 {
        assert!(byte_idx < 32);
        self.0.byte(byte_idx)
    }

    pub fn bit(&self, bit_idx: usize) -> bool {
        self.0.bit(bit_idx)
    }

    pub fn as_le_bytes_ref(&self) -> &[u8; 32] {
        self.0.as_le_bytes()
    }

    pub fn reduce_mod(&mut self, modulus: &Self) {
        if modulus.is_zero() {
            Self::write_zero(self);
            return;
        }
        if (&*self) >= modulus {
            let mut modulus = modulus.clone();
            Self::div_rem(self, &mut modulus);
            self.clone_from(&modulus);
        }
    }

    pub fn add_mod(a: &mut Self, b: &mut Self, modulus_or_result: &mut Self) {
        a.reduce_mod(&modulus_or_result);
        b.reduce_mod(&modulus_or_result);

        let of = unsafe { bigint_op_delegation::<ADD_OP_BIT_IDX>(&mut a.0, &b.0) != 0 };

        if of || a >= modulus_or_result {
            unsafe { bigint_op_delegation::<SUB_OP_BIT_IDX>(&mut a.0, &modulus_or_result.0) };
        }

        modulus_or_result.clone_from(a);
    }

    pub fn mul_mod(a: &mut Self, b: &mut Self, modulus_or_result: &mut Self) {
        if modulus_or_result.0.is_zero_mut() {
            return;
        }

        #[cfg(target_arch = "riscv32")]
        {
            oracle_mulmod(a, b, modulus_or_result);
        }

        #[cfg(not(target_arch = "riscv32"))]
        {
            let mut product = [0u64; 8];
            let _ = ruint::algorithms::addmul(&mut product, a.as_limbs(), b.as_limbs());
            ruint::algorithms::div(&mut product, modulus_or_result.as_limbs_mut());
        }
    }

    pub fn pow(base: &Self, exp: &Self, dst: &mut Self) {
        // Exponentiation by squaring
        Self::write_one(dst);
        let bits = crate::BitIteratorBE::new_without_leading_zeros(exp.as_limbs());
        for i in bits {
            let tmp = dst.clone();
            Self::wrapping_mul_assign(dst, &tmp);

            if i {
                Self::wrapping_mul_assign(dst, &base);
            }
        }
    }

    pub fn byte_len(&self) -> usize {
        (self.bit_len() + 7) / 8
    }

    pub fn checked_add(&self, rhs: &Self) -> Option<Self> {
        let mut result = self.clone();
        let of = result.overflowing_add_assign(rhs);
        if of {
            None
        } else {
            Some(result)
        }
    }

    pub fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        let mut result = self.clone();
        let of = result.overflowing_sub_assign(rhs);
        if of {
            None
        } else {
            Some(result)
        }
    }

    pub fn checked_mul(&self, rhs: &Self) -> Option<Self> {
        let mut result = self.clone();
        let of = result.0.mul_low_assign(&rhs.0);

        if of {
            None
        } else {
            Some(result)
        }
    }
}

impl From<ruint::aliases::U256> for U256 {
    #[inline(always)]
    fn from(value: ruint::aliases::U256) -> Self {
        // NOTE: we can not use precompile call due to alignment requirements
        Self::from_limbs(*value.as_limbs())
    }
}

impl From<u64> for U256 {
    #[inline(always)]
    fn from(value: u64) -> Self {
        Self(DelegatedU256::from(value))
    }
}

impl From<u32> for U256 {
    #[inline(always)]
    fn from(value: u32) -> Self {
        Self(DelegatedU256::from(value))
    }
}

impl From<u128> for U256 {
    #[inline(always)]
    fn from(value: u128) -> Self {
        Self(DelegatedU256::from(value))
    }
}

impl Into<ruint::aliases::U256> for U256 {
    #[inline(always)]
    fn into(self) -> ruint::aliases::U256 {
        ruint::aliases::U256::from_limbs(self.0.to_limbs())
    }
}

impl TryInto<usize> for U256 {
    type Error = ruint::FromUintError<()>;

    fn try_into(self) -> Result<usize, Self::Error> {
        let limbs = self.0.to_limbs();
        if limbs[3] != 0 || limbs[2] != 0 || limbs[1] != 0 {
            Err(ruint::FromUintError::Overflow(usize::BITS as usize, (), ()))
        } else {
            if limbs[0] > usize::MAX as u64 {
                Err(ruint::FromUintError::Overflow(usize::BITS as usize, (), ()))
            } else {
                Ok(limbs[0] as usize)
            }
        }
    }
}

impl TryInto<u64> for U256 {
    type Error = ruint::FromUintError<()>;

    fn try_into(self) -> Result<u64, Self::Error> {
        let limbs = self.0.to_limbs();
        if limbs[3] != 0 || limbs[2] != 0 || limbs[1] != 0 {
            Err(ruint::FromUintError::Overflow(usize::BITS as usize, (), ()))
        } else {
            Ok(limbs[0])
        }
    }
}

impl<'a> AddAssign<&'a U256> for U256 {
    #[inline(always)]
    fn add_assign(&mut self, rhs: &'a U256) {
        let _ = self.overflowing_add_assign(rhs);
    }
}

impl<'a> SubAssign<&'a U256> for U256 {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: &'a U256) {
        let _ = self.overflowing_sub_assign(rhs);
    }
}

impl<'a> BitXorAssign<&'a U256> for U256 {
    #[inline(always)]
    fn bitxor_assign(&mut self, rhs: &'a U256) {
        self.0 ^= &rhs.0;
    }
}

impl<'a> BitAndAssign<&'a U256> for U256 {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: &'a U256) {
        self.0 &= &rhs.0;
    }
}

impl<'a> BitOrAssign<&'a U256> for U256 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: &'a U256) {
        self.0 |= &rhs.0;
    }
}

impl ShrAssign<u32> for U256 {
    #[inline(always)]
    fn shr_assign(&mut self, rhs: u32) {
        self.0 >>= rhs;
    }
}

impl ShlAssign<u32> for U256 {
    fn shl_assign(&mut self, rhs: u32) {
        self.0 <<= rhs;
    }
}

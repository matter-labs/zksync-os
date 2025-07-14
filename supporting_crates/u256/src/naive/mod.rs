use core::ops::*;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct U256(ruint::aliases::U256);

impl Clone for U256 {
    #[inline(always)]
    fn clone(&self) -> Self {
        // copy
        Self(self.0)
    }

    #[inline(always)]
    fn clone_from(&mut self, source: &Self) {
        self.0 = source.0;
    }
}

impl core::fmt::Display for U256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::LowerHex::fmt(self, f)
    }
}

impl core::fmt::Debug for U256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::LowerHex::fmt(self, f)
    }
}

impl core::fmt::LowerHex for U256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for word in self.as_limbs().iter().rev() {
            write!(f, "{:016x}", word)?;
        }

        core::fmt::Result::Ok(())
    }
}

impl U256 {
    pub const ZERO: Self = Self(ruint::aliases::U256::ZERO);
    pub const ONE: Self = Self(ruint::aliases::U256::ONE);

    pub const BYTES: usize = 32;

    pub const fn from_limbs(limbs: [u64; 4]) -> Self {
        Self(ruint::aliases::U256::from_limbs(limbs))
    }

    /// # Safety
    /// `dst` must by 32 byte aligned and point to 32 bytes of accessible memory.
    pub unsafe fn write_into_ptr(dst: *mut Self, source: &Self) {
        unsafe {
            dst.write(Self(source.0));
        }
    }

    #[inline(always)]
    pub fn zero() -> Self {
        Self::ZERO
    }

    #[inline(always)]
    pub fn one() -> Self {
        Self::ONE
    }

    pub fn bytereverse(&mut self) {
        unsafe {
            let limbs = self.as_limbs_mut();
            core::ptr::swap(&mut limbs[0] as *mut u64, &mut limbs[3] as *mut u64);
            core::ptr::swap(&mut limbs[1] as *mut u64, &mut limbs[2] as *mut u64);
            for limb in limbs.iter_mut() {
                *limb = limb.swap_bytes();
            }
        }
    }

    #[inline(always)]
    pub fn write_zero(into: &mut Self) {
        *into = Self::ZERO;
    }

    #[inline(always)]
    pub fn write_one(into: &mut Self) {
        *into = Self::ONE;
    }

    #[inline(always)]
    pub const fn as_limbs(&self) -> &[u64; 4] {
        self.0.as_limbs()
    }

    #[inline(always)]
    pub fn as_limbs_mut(&mut self) -> &mut [u64; 4] {
        unsafe { self.0.as_limbs_mut() }
    }

    #[inline(always)]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    #[inline(always)]
    pub fn is_one(&self) -> bool {
        self.0 == ruint::aliases::U256::ONE
    }

    #[inline(always)]
    pub fn overflowing_add_assign(&mut self, rhs: &Self) -> bool {
        let (t, of) = self.0.overflowing_add(rhs.0);
        self.0 = t;

        of
    }

    #[inline(always)]
    pub fn overflowing_add(self, rhs: Self) -> (Self, bool) {
        let (t, of) = self.0.overflowing_add(rhs.0);
        (Self(t), of)
    }

    #[inline(always)]
    pub fn overflowing_sub_assign(&mut self, rhs: &Self) -> bool {
        let (t, of) = self.0.overflowing_sub(rhs.0);
        self.0 = t;

        of
    }

    #[inline(always)]
    pub fn overflowing_sub(self, rhs: Self) -> (Self, bool) {
        let (t, of) = self.0.overflowing_sub(rhs.0);
        (Self(t), of)
    }

    #[inline(always)]
    pub fn overflowing_sub_assign_reversed(&mut self, rhs: &Self) -> bool {
        let (t, of) = rhs.0.overflowing_sub(self.0);
        self.0 = t;

        of
    }

    #[inline(always)]
    pub fn wrapping_mul_assign(&mut self, rhs: &Self) {
        self.0 = self.0.wrapping_mul(rhs.0);
    }

    #[inline(always)]
    pub fn high_mul_assign(&mut self, rhs: &Self) {
        let t: ruint::aliases::U512 = self.0.widening_mul(rhs.0);
        self.as_limbs_mut().copy_from_slice(&t.as_limbs()[4..8]);
    }

    #[inline(always)]
    /// Panics if divisor is 0
    pub fn div_rem(dividend_or_quotient: &mut Self, divisor_or_remainder: &mut Self) {
        let (q, r) = dividend_or_quotient.0.div_rem(divisor_or_remainder.0);
        dividend_or_quotient.0 = q;
        divisor_or_remainder.0 = r;
    }

    #[inline(always)]
    /// Panics if divisor is 0
    pub fn div_ceil(dividend_or_quotient: &mut Self, divisor: &Self) {
        let result = dividend_or_quotient.0.div_ceil(divisor.0);
        dividend_or_quotient.0 = result;
    }

    #[inline(always)]
    pub fn not_mut(&mut self) {
        self.0 = !self.0;
    }

    pub fn try_from_be_slice(input: &[u8]) -> Option<Self> {
        match input.try_into() {
            Ok(bytes) => Some(Self::from_be_bytes(bytes)),
            Err(_) => None,
        }
    }

    pub fn from_be_bytes(input: &[u8; 32]) -> Self {
        Self(ruint::aliases::U256::from_be_bytes::<32>(*input))
    }

    pub fn from_le_bytes(input: &[u8; 32]) -> Self {
        Self(ruint::aliases::U256::from_le_bytes::<32>(*input))
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

    #[inline(always)]
    /// # Safety
    /// `into` must be 32 byte aligned and point to 32 bytes of accessible memory
    pub unsafe fn write_zero_into_ptr(into: *mut Self) {
        unsafe {
            into.write(Self::ZERO);
        }
    }

    #[inline(always)]
    /// # Safety
    /// `into` must be 32 byte aligned and point to 32 bytes of accessible memory
    pub unsafe fn write_one_into_ptr(into: *mut Self) {
        unsafe {
            into.write(Self::ONE);
        }
    }

    pub fn byte(&self, byte_idx: usize) -> u8 {
        self.0.byte(byte_idx)
    }

    pub fn bit(&self, bit_idx: usize) -> bool {
        self.0.bit(bit_idx)
    }

    pub fn as_le_bytes_ref(&self) -> &[u8; 32] {
        unsafe { core::mem::transmute(self.0.as_limbs()) }
    }

    pub fn add_mod(a: &mut Self, b: &mut Self, modulus_or_result: &mut Self) {
        modulus_or_result.0 = ruint::aliases::U256::add_mod(a.0, b.0, modulus_or_result.0);
    }

    pub fn mul_mod(a: &mut Self, b: &mut Self, modulus_or_result: &mut Self) {
        if modulus_or_result.is_zero() {
            return;
        }

        let mut product = [0u64; 8];
        let _ = ruint::algorithms::addmul(&mut product, a.as_limbs(), b.as_limbs());

        ruint::algorithms::div(&mut product, modulus_or_result.as_limbs_mut());
    }

    pub fn pow(base: &Self, exp: &Self, dst: &mut Self) {
        dst.0 = base.0.pow(exp.0);
    }

    pub fn byte_len(&self) -> usize {
        self.0.byte_len()
    }

    pub fn checked_add(&self, rhs: &Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    pub fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }

    pub fn checked_mul(&self, rhs: &Self) -> Option<Self> {
        self.0.checked_mul(rhs.0).map(Self)
    }
}

impl From<ruint::aliases::U256> for U256 {
    #[inline(always)]
    fn from(value: ruint::aliases::U256) -> Self {
        Self(value)
    }
}

impl From<u64> for U256 {
    #[inline(always)]
    fn from(value: u64) -> Self {
        let mut result = Self::zero();
        result.as_limbs_mut()[0] = value;

        result
    }
}

impl From<u32> for U256 {
    #[inline(always)]
    fn from(value: u32) -> Self {
        let mut result = Self::zero();
        result.as_limbs_mut()[0] = value as u64;

        result
    }
}

impl From<u128> for U256 {
    #[inline(always)]
    fn from(value: u128) -> Self {
        let mut result = Self::zero();
        result.as_limbs_mut()[0] = value as u64;
        result.as_limbs_mut()[1] = (value >> 64) as u64;

        result
    }
}

impl From<U256> for ruint::aliases::U256 {
    fn from(value: U256) -> Self {
        value.0
    }
}

impl TryInto<usize> for U256 {
    type Error = ruint::FromUintError<()>;

    fn try_into(self) -> Result<usize, Self::Error> {
        if self.as_limbs()[3] != 0
            || self.as_limbs()[2] != 0
            || self.as_limbs()[1] != 0
            || self.as_limbs()[0] > usize::MAX as u64
        {
            Err(ruint::FromUintError::Overflow(usize::BITS as usize, (), ()))
        } else {
            Ok(self.as_limbs()[0] as usize)
        }
    }
}

impl TryInto<u64> for U256 {
    type Error = ruint::FromUintError<()>;

    fn try_into(self) -> Result<u64, Self::Error> {
        if self.as_limbs()[3] != 0 || self.as_limbs()[2] != 0 || self.as_limbs()[1] != 0 {
            Err(ruint::FromUintError::Overflow(usize::BITS as usize, (), ()))
        } else {
            Ok(self.as_limbs()[0])
        }
    }
}

// we only provide a small set of operations in the mutable form to avoid excessive copies

impl<'a> AddAssign<&'a U256> for U256 {
    #[inline(always)]
    fn add_assign(&mut self, rhs: &'a U256) {
        self.0.add_assign(&rhs.0);
    }
}

impl<'a> SubAssign<&'a U256> for U256 {
    #[inline(always)]
    fn sub_assign(&mut self, rhs: &'a U256) {
        self.0.sub_assign(&rhs.0);
    }
}

impl<'a> BitXorAssign<&'a U256> for U256 {
    #[inline(always)]
    fn bitxor_assign(&mut self, rhs: &'a U256) {
        self.0.bitxor_assign(&rhs.0);
    }
}

impl<'a> BitAndAssign<&'a U256> for U256 {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: &'a U256) {
        self.0.bitand_assign(&rhs.0);
    }
}

impl<'a> BitOrAssign<&'a U256> for U256 {
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: &'a U256) {
        self.0.bitor_assign(&rhs.0);
    }
}

impl ShrAssign<u32> for U256 {
    #[inline(always)]
    fn shr_assign(&mut self, rhs: u32) {
        self.0.shr_assign(rhs);
    }
}

impl ShlAssign<u32> for U256 {
    #[inline(always)]
    fn shl_assign(&mut self, rhs: u32) {
        self.0.shl_assign(rhs);
    }
}

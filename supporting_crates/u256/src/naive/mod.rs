use core::ops::*;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[repr(transparent)]
pub struct U256(ruint::aliases::U256);

#[cfg(not(all(target_arch = "riscv32", feature = "delegation")))]
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

impl U256 {
    const ZERO: Self = Self(ruint::aliases::U256::ZERO);
    const ONE: Self = Self(ruint::aliases::U256::ONE);

    #[inline(always)]
    pub fn zero() -> Self {
        Self::ZERO
    }

    #[inline(always)]
    pub fn one() -> Self {
        Self::ONE
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
    pub fn overflowing_sub_assign(&mut self, rhs: &Self) -> bool {
        let (t, of) = self.0.overflowing_sub(rhs.0);
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
}

impl From<ruint::aliases::U256> for U256 {
    #[inline(always)]
    fn from(value: ruint::aliases::U256) -> Self {
        Self(value)
    }
}

impl Into<ruint::aliases::U256> for U256 {
    #[inline(always)]
    fn into(self) -> ruint::aliases::U256 {
        self.0
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

impl Not for U256 {
    type Output = Self;

    #[inline(always)]
    fn not(self) -> Self::Output {
        Self(self.0.not())
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

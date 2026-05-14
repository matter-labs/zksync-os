macro_rules! impl_conversions {
    ($U256:ty) => {
        impl $U256 {
            #[inline(always)]
            pub fn to_b160(&self) -> ruint::aliases::B160 {
                let mut result = ruint::aliases::B160::ZERO;
                unsafe {
                    result.as_limbs_mut()[0] = self.as_limbs()[0];
                    result.as_limbs_mut()[1] = self.as_limbs()[1];
                    result.as_limbs_mut()[2] = self.as_limbs()[2] & 0x00000000ffffffff;
                }
                result
            }

            #[inline(always)]
            pub fn from_b160(src: ruint::aliases::B160) -> Self {
                let mut result = Self::zero();
                result.as_limbs_mut()[0] = src.as_limbs()[0];
                result.as_limbs_mut()[1] = src.as_limbs()[1];
                result.as_limbs_mut()[2] = src.as_limbs()[2];
                result
            }

            #[inline(always)]
            pub fn to_u64_saturated(&self) -> u64 {
                let limbs = self.as_limbs();
                if limbs[3] != 0 || limbs[2] != 0 || limbs[1] != 0 {
                    u64::MAX
                } else {
                    limbs[0]
                }
            }

            #[inline(always)]
            pub fn try_to_usize(&self) -> Option<usize> {
                let limbs = self.as_limbs();
                if limbs[3] != 0 || limbs[2] != 0 || limbs[1] != 0 {
                    None
                } else {
                    limbs[0].try_into().ok()
                }
            }

            #[inline(always)]
            pub fn try_to_usize_capped<const CAP: usize>(&self) -> Option<usize> {
                let limbs = self.as_limbs();
                if limbs[3] != 0 || limbs[2] != 0 || limbs[1] != 0 || limbs[0] >= CAP as u64 {
                    None
                } else {
                    Some(limbs[0] as usize)
                }
            }

            #[inline(always)]
            pub fn to_usize_saturated(&self) -> usize {
                let value = self.to_u64_saturated();
                if cfg!(target_pointer_width = "32") {
                    if value > u32::MAX as u64 {
                        u32::MAX as usize
                    } else {
                        value as usize
                    }
                } else {
                    value as usize
                }
            }
        }
    };
}

pub(crate) use impl_conversions;

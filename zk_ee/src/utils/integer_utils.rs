use ruint::{
    aliases::{B160, B256, U256},
    Bits,
};

#[repr(transparent)]
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct BitsOrd<const BITS: usize, const LIMBS: usize>(pub Bits<BITS, LIMBS>);

impl<const BITS: usize, const LIMBS: usize> AsRef<Bits<BITS, LIMBS>> for BitsOrd<BITS, LIMBS> {
    fn as_ref(&self) -> &Bits<BITS, LIMBS> {
        &self.0
    }
}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl<const BITS: usize, const LIMBS: usize> PartialOrd for BitsOrd<BITS, LIMBS> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.0.as_limbs().partial_cmp(&other.0.as_limbs())
    }
}

impl<const BITS: usize, const LIMBS: usize> Ord for BitsOrd<BITS, LIMBS> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.as_limbs().cmp(&other.0.as_limbs())
    }
}

impl<const BITS: usize, const LIMBS: usize> From<Bits<BITS, LIMBS>> for BitsOrd<BITS, LIMBS> {
    fn from(value: Bits<BITS, LIMBS>) -> Self {
        Self(value)
    }
}

impl<const BITS: usize, const LIMBS: usize> From<&Bits<BITS, LIMBS>> for &BitsOrd<BITS, LIMBS> {
    fn from(value: &Bits<BITS, LIMBS>) -> Self {
        unsafe { &*(value as *const _ as *const _) }
    }
}

#[inline(always)]
pub fn u256_to_u8_checked(src: U256) -> u8 {
    assert!(src.as_limbs()[3] == 0 && src.as_limbs()[2] == 0 && src.as_limbs()[1] == 0);
    assert!(src.as_limbs()[0] < (1u64 << 8));

    src.as_limbs()[0] as u8
}

#[inline(always)]
pub fn b256_to_u256(src: B256) -> U256 {
    U256::from_be_bytes(src.to_be_bytes::<{ B256::BYTES }>())
}

#[inline(always)]
pub fn u256_to_u64_saturated(src: &U256) -> u64 {
    let limbs = src.as_limbs();
    if limbs[3] != 0 || limbs[2] != 0 || limbs[1] != 0 {
        u64::MAX
    } else {
        limbs[0]
    }
}

#[inline(always)]
pub fn u256_try_to_u64(src: &U256) -> Option<u64> {
    let limbs = src.as_limbs();
    if limbs[3] != 0 || limbs[2] != 0 || limbs[1] != 0 {
        None
    } else {
        Some(limbs[0])
    }
}

#[inline(always)]
pub fn u256_try_to_usize_capped<const CAP: usize>(src: &U256) -> Option<usize> {
    let limbs = src.as_limbs();
    if limbs[3] != 0 || limbs[2] != 0 || limbs[1] != 0 || limbs[0] >= CAP as u64 {
        None
    } else {
        Some(limbs[0] as usize)
    }
}

#[inline(always)]
pub fn u256_to_usize_saturated(src: &U256) -> usize {
    u256_to_u64_saturated(src) as usize
}

#[inline(always)]
pub fn u256_try_to_usize(src: &U256) -> Option<usize> {
    let limbs = src.as_limbs();
    if limbs[3] != 0 || limbs[2] != 0 || limbs[1] != 0 {
        None
    } else {
        limbs[0].try_into().ok()
    }
}

#[inline(always)]
pub fn u256_to_b160(src: &U256) -> B160 {
    let mut result = B160::ZERO;
    unsafe {
        result.as_limbs_mut()[0] = src.as_limbs()[0];
        result.as_limbs_mut()[1] = src.as_limbs()[1];
        result.as_limbs_mut()[2] = src.as_limbs()[2] & 0x00000000ffffffff;
    }

    result
}

#[inline(always)]
pub fn b160_to_u256(src: B160) -> U256 {
    let mut result = U256::ZERO;
    unsafe {
        result.as_limbs_mut()[0] = src.as_limbs()[0];
        result.as_limbs_mut()[1] = src.as_limbs()[1];
        result.as_limbs_mut()[2] = src.as_limbs()[2];
    }

    result
}

#[inline(always)]
pub fn u256_to_b160_checked(src: U256) -> B160 {
    assert!(src.as_limbs()[3] == 0 && src.as_limbs()[2] < (1u64 << 32));
    let mut result = B160::ZERO;
    unsafe {
        result.as_limbs_mut()[0] = src.as_limbs()[0];
        result.as_limbs_mut()[1] = src.as_limbs()[1];
        result.as_limbs_mut()[2] = src.as_limbs()[2];
    }

    result
}

#[inline(always)]
pub fn u256_try_to_b160(src: U256) -> Option<B160> {
    if src.as_limbs()[3] != 0 || src.as_limbs()[2] >= (1u64 << 32) {
        return None;
    }
    let mut result = B160::ZERO;
    unsafe {
        result.as_limbs_mut()[0] = src.as_limbs()[0];
        result.as_limbs_mut()[1] = src.as_limbs()[1];
        result.as_limbs_mut()[2] = src.as_limbs()[2];
    }

    Some(result)
}

#[cfg(test)]
mod tests {

    use ruint::aliases::U160;

    #[test]
    fn bits_ord_limb_single() {
        let a = U160::from(1);
        let b = a + a;

        println!("a: {:0x?}, {:0x?}", a, a.as_limbs());
        println!("b: {:0x?}, {:0x?}", b, b.as_limbs());

        assert!(b > a);
    }

    #[test]
    fn bits_ord_limb_span() {
        let a = U160::from_limbs([u64::MAX, 0, 0]);
        let b = U160::from(1);

        let c = a + b;

        println!("a: {:0x?}, {:0x?}", a, a.as_limbs());
        println!("b: {:0x?}, {:0x?}", b, b.as_limbs());
        println!("c: {:0x?}, {:0x?}", c, c.as_limbs());

        assert!(c > a);
    }
}

use super::WordLayout;
use ruint::aliases::{B160, U256};

impl WordLayout for U256 {
    const WORD_COUNT: Option<usize> = Some(8);

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        for limb in self.as_limbs() {
            limb.write_words(w);
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let limbs: [u64; 4] = core::array::from_fn(|_| u64::read_words(r));
        Self::from_limbs(limbs)
    }
}

impl WordLayout for B160 {
    const WORD_COUNT: Option<usize> = Some(6);

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        for limb in self.as_limbs() {
            limb.write_words(w);
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let limbs: [u64; 3] = core::array::from_fn(|_| u64::read_words(r));
        Self::from_limbs(limbs)
    }
}

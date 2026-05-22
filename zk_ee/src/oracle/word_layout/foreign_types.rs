use super::WordLayout;
use ruint::aliases::{B160, U256};

impl WordLayout for U256 {
    const WORD_COUNT: Option<usize> = Some(8);

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        let src = self as *const Self as *const u32;
        for i in 0..8 {
            w(unsafe { src.add(i).read() });
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let mut result = core::mem::MaybeUninit::<Self>::uninit();
        let dst = result.as_mut_ptr() as *mut u32;
        for i in 0..8 {
            unsafe { dst.add(i).write(r()) };
        }
        unsafe { result.assume_init() }
    }
}

impl WordLayout for B160 {
    const WORD_COUNT: Option<usize> = Some(6);

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        let src = self as *const Self as *const u32;
        for i in 0..6 {
            w(unsafe { src.add(i).read() });
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let mut result = core::mem::MaybeUninit::<Self>::uninit();
        let dst = result.as_mut_ptr() as *mut u32;
        for i in 0..6 {
            unsafe { dst.add(i).write(r()) };
        }
        let result = unsafe { result.assume_init() };
        assert!(
            result.as_limbs()[2] >> 32 == 0,
            "B160 value has non-zero bits above 160"
        );
        result
    }
}

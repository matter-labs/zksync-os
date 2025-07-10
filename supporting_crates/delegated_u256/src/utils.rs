use core::mem::MaybeUninit;

use super::DelegatedU256;

impl DelegatedU256 {
    pub const fn as_limbs(&self) -> &[u64; 4] {
        &self.0
    }

    pub const fn as_limbs_mut(&mut self) -> &mut [u64; 4] {
        &mut self.0
    }

    pub const fn to_limbs(self) -> [u64; 4] {
        self.0
    }

    pub const fn from_limbs(limbs: [u64; 4]) -> Self {
        Self(limbs)
    }

    pub const fn from_be_bytes(input: &[u8; 32]) -> Self {
        unsafe {
            #[allow(invalid_value)]
            #[allow(clippy::uninit_assumed_init)]
            // `result.assume_init()` may trigger stack-to-stack copy, so we can't do it later
            // This is safe because there are no references to result and it's initialized immediately
            // (and on RISC-V all memory is init by default)
            let mut result: DelegatedU256 = MaybeUninit::uninit().assume_init();
            let ptr = &mut result.0[0] as *mut u64;
            let src: *const [u8; 8] = input.as_ptr_range().end.cast();

            ptr.write(u64::from_be_bytes(src.sub(1).read()));
            ptr.add(1).write(u64::from_be_bytes(src.sub(2).read()));
            ptr.add(2).write(u64::from_be_bytes(src.sub(3).read()));
            ptr.add(3).write(u64::from_be_bytes(src.sub(4).read()));

            result
        }
    }

    pub fn to_be_bytes(&self) -> [u8; 32] {
        let mut res = self.clone();
        res.bytereverse();
        unsafe { core::mem::transmute(res) }
    }

    pub fn from_le_bytes(input: &[u8; 32]) -> Self {
        unsafe {
            #[allow(invalid_value)]
            #[allow(clippy::uninit_assumed_init)]
            // `result.assume_init()` may trigger stack-to-stack copy, so we can't do it later
            // This is safe because there are no references to result and it's initialized immediately
            // (and on RISC-V all memory is init by default)
            let mut result: DelegatedU256 = MaybeUninit::uninit().assume_init();
            let ptr = &mut result.0[0] as *mut u64;
            let src: *const [u8; 8] = input.as_ptr().cast();

            ptr.write(u64::from_le_bytes(src.read()));
            ptr.add(1).write(u64::from_le_bytes(src.add(1).read()));
            ptr.add(2).write(u64::from_le_bytes(src.add(2).read()));
            ptr.add(3).write(u64::from_le_bytes(src.add(3).read()));

            result
        }
    }

    pub fn to_le_bytes(&self) -> [u8; 32] {
        unsafe { core::mem::transmute(self.clone()) }
    }

    pub fn as_le_bytes(&self) -> &[u8; 32] {
        unsafe { core::mem::transmute(&self.0) }
    }

    pub fn bytereverse(&mut self) {
        let limbs = self.as_limbs_mut();
        unsafe {
            core::ptr::swap(&mut limbs[0] as *mut u64, &mut limbs[3] as *mut u64);
            core::ptr::swap(&mut limbs[1] as *mut u64, &mut limbs[2] as *mut u64);
        }
        for limb in limbs.iter_mut() {
            *limb = limb.swap_bytes();
        }
    }

    pub fn bit_len(&self) -> usize {
        let mut len = 256usize;
        for el in self.0.iter().rev() {
            if *el == 0 {
                len -= 64;
            } else {
                len -= el.leading_zeros() as usize;
                return len;
            }
        }

        debug_assert!(len == 0);
        debug_assert!(self.is_zero());

        len
    }

    pub fn byte(&self, byte_idx: usize) -> u8 {
        if byte_idx >= 32 {
            0
        } else {
            self.as_le_bytes()[byte_idx]
        }
    }

    pub fn bit(&self, bit_idx: usize) -> bool {
        if bit_idx >= 256 {
            false
        } else {
            let (word, bit_idx) = (bit_idx / 64, bit_idx % 64);
            self.0[word] & 1 << bit_idx != 0
        }
    }
}

impl core::fmt::Display for DelegatedU256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::LowerHex::fmt(self, f)
    }
}

impl core::fmt::Debug for DelegatedU256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::LowerHex::fmt(self, f)
    }
}

impl core::fmt::LowerHex for DelegatedU256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for word in self.as_limbs().iter().rev() {
            write!(f, "{:016x}", word)?;
        }

        core::fmt::Result::Ok(())
    }
}

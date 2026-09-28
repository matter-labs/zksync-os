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

    #[inline(always)]
    pub fn from_be_bytes(input: &[u8; 32]) -> Self {
        unsafe {
            let mut result = MaybeUninit::<DelegatedU256>::uninit();
            Self::write_be_bytes_into_slot(input.as_ptr(), result.as_mut_ptr());
            result.assume_init()
        }
    }

    /// The big-endian bytes of the value. Without delegation, RISC-V returns the
    /// `rev8`-reversed words by value, with no branch or scratch buffer, so the
    /// caller's copy can stay in registers.
    #[inline(always)]
    pub fn to_be_bytes(&self) -> [u8; 32] {
        #[cfg(all(target_arch = "riscv32", not(feature = "bytereverse_delegation")))]
        // SAFETY: `self` is an aligned, initialized slot of eight words
        unsafe {
            crate::byte_order::words_to_be_bytes_32((self as *const Self).cast::<u32>())
        }

        #[cfg(not(all(target_arch = "riscv32", not(feature = "bytereverse_delegation"))))]
        {
            let mut out = MaybeUninit::<[u8; 32]>::uninit();
            // SAFETY: `out` is 32 writable bytes, fully written below
            unsafe {
                self.write_be_bytes_into(&mut *out.as_mut_ptr());
                out.assume_init()
            }
        }
    }

    /// Writes the big-endian bytes of the value into `dst`. RISC-V uses a reversed
    /// scratch copy and a plain copy with delegation, or stores the by-value
    /// [`Self::to_be_bytes`] as a byte-aligned `[u8; 32]` without it. For destinations
    /// inside heap memory, use [`Self::write_slot_as_be_bytes`] to get word stores
    /// when aligned. Other targets use a reversed byte-by-byte copy.
    #[inline(always)]
    pub fn write_be_bytes_into(&self, dst: &mut [u8; 32]) {
        #[cfg(all(target_arch = "riscv32", feature = "bytereverse_delegation"))]
        {
            let mut reversed = self.clone();
            reversed.bytereverse_and_write_le(dst);
        }

        #[cfg(all(target_arch = "riscv32", not(feature = "bytereverse_delegation")))]
        {
            *dst = self.to_be_bytes();
        }

        #[cfg(not(target_arch = "riscv32"))]
        // SAFETY: `self` is 32 initialized bytes, `dst` is 32 writable bytes
        unsafe {
            Self::reversed_byte_copy((self as *const Self).cast::<u8>(), dst.as_mut_ptr());
        }
    }

    /// Writes the big-endian bytes of the value into `dst`. With delegation on
    /// RISC-V, `self` is reversed in place and copied out, so the caller must no
    /// longer need it. Without delegation, RISC-V reverses aligned words with
    /// `rev8` (shifted words for an unaligned `dst`), leaving `self` untouched.
    /// Other targets also leave `self` untouched and copy bytes in reverse order.
    #[inline(always)]
    pub fn bytereverse_and_write_le(&mut self, dst: &mut [u8; 32]) {
        #[cfg(all(target_arch = "riscv32", feature = "bytereverse_delegation"))]
        {
            self.bytereverse();
            // SAFETY: `self` is an initialized slot, `dst` is 32 writable bytes
            unsafe { Self::copy_slot_to_bytes(self as *const Self, dst.as_mut_ptr()) }
        }

        // The callers pass word-aligned `Bytes32` locals, where the alignment test
        // folds away; the by-value form costs two callee-saved registers in SSTORE.
        #[cfg(all(target_arch = "riscv32", not(feature = "bytereverse_delegation")))]
        // SAFETY: `self` is an aligned, initialized slot; `dst` is a distinct
        // writable array, and the helper handles unaligned destinations.
        unsafe {
            crate::byte_order::write_words_as_be_32(
                (self as *const Self).cast::<u32>(),
                dst.as_mut_ptr(),
            );
        }

        #[cfg(not(target_arch = "riscv32"))]
        // SAFETY: `self` is 32 initialized bytes, `dst` is 32 writable bytes
        unsafe {
            Self::reversed_byte_copy((self as *const Self).cast::<u8>(), dst.as_mut_ptr());
        }
    }

    pub fn from_le_bytes(input: &[u8; 32]) -> Self {
        unsafe {
            let mut result = MaybeUninit::<DelegatedU256>::uninit();
            let ptr = result.as_mut_ptr().cast::<u64>();
            let src: *const [u8; 8] = input.as_ptr().cast();

            ptr.write(u64::from_le_bytes(src.read()));
            ptr.add(1).write(u64::from_le_bytes(src.add(1).read()));
            ptr.add(2).write(u64::from_le_bytes(src.add(2).read()));
            ptr.add(3).write(u64::from_le_bytes(src.add(3).read()));

            result.assume_init()
        }
    }

    pub fn to_le_bytes(&self) -> [u8; 32] {
        unsafe { core::mem::transmute(self.clone()) }
    }

    pub fn as_le_bytes(&self) -> &[u8; 32] {
        unsafe { core::mem::transmute(&self.0) }
    }

    pub fn bytereverse(&mut self) {
        // SAFETY: `self` is an aligned, initialized value
        unsafe { Self::bytereverse_in_place(self as *mut Self) }
    }

    /// Reverses the 32 bytes of the value at `ptr`: by delegation on RISC-V
    /// when enabled, by word reversal with `rev8` otherwise on RISC-V, and by
    /// swapping and reversing limbs on other targets.
    ///
    /// # Safety
    /// `ptr` must be 32 bytes aligned and point to 32 bytes of initialized memory.
    #[inline(always)]
    pub unsafe fn bytereverse_in_place(ptr: *mut Self) {
        #[cfg(all(target_arch = "riscv32", feature = "bytereverse_delegation"))]
        unsafe {
            // the immutable operand is read but unused; a static is a valid one
            let _ = crate::delegation::bigint_op_delegation::<
                { crate::delegation::BYTEREVERSE_OP_BIT_IDX },
            >(ptr, core::ptr::addr_of!(crate::arithmetic::ZERO));
        }

        #[cfg(all(target_arch = "riscv32", not(feature = "bytereverse_delegation")))]
        unsafe {
            crate::byte_order::bytereverse_words_in_place::<8>(ptr.cast::<u32>());
        }

        #[cfg(not(target_arch = "riscv32"))]
        unsafe {
            let limbs = (*ptr).as_limbs_mut();
            core::ptr::swap(&mut limbs[0] as *mut u64, &mut limbs[3] as *mut u64);
            core::ptr::swap(&mut limbs[1] as *mut u64, &mut limbs[2] as *mut u64);
            for limb in limbs.iter_mut() {
                *limb = limb.swap_bytes();
            }
        }
    }

    /// Copies 32 bytes from `src` (any alignment) into the slot `dst`: by one memcopy
    /// delegation when the source is 32 bytes aligned, by words when it is 4 bytes
    /// aligned, by bytes otherwise.
    ///
    /// # Safety
    /// `src` must be readable for 32 bytes, `dst` must be 32 bytes aligned and writable.
    #[inline(always)]
    pub unsafe fn copy_bytes_into_slot(src: *const u8, dst: *mut Self) {
        unsafe {
            if src.addr().is_multiple_of(32) {
                let _ = crate::delegation::bigint_op_delegation::<
                    { crate::delegation::MEMCOPY_BIT_IDX },
                >(dst, src.cast::<Self>());
            } else if src.addr().is_multiple_of(4) {
                let src = src.cast::<u32>();
                let dst = dst.cast::<u32>();
                for i in 0..8 {
                    dst.add(i).write(src.add(i).read());
                }
            } else {
                let dst = dst.cast::<u8>();
                for i in 0..32 {
                    dst.add(i).write(src.add(i).read());
                }
            }
        }
    }

    /// Copies the 32 bytes of the slot `src` to `dst` (any alignment); the mirror of
    /// [`Self::copy_bytes_into_slot`].
    ///
    /// # Safety
    /// `src` must be 32 bytes aligned and initialized, `dst` must be writable for 32 bytes.
    #[inline(always)]
    pub unsafe fn copy_slot_to_bytes(src: *const Self, dst: *mut u8) {
        unsafe {
            if dst.addr().is_multiple_of(32) {
                let _ = crate::delegation::bigint_op_delegation::<
                    { crate::delegation::MEMCOPY_BIT_IDX },
                >(dst.cast::<Self>(), src);
            } else if dst.addr().is_multiple_of(4) {
                let src = src.cast::<u32>();
                let dst = dst.cast::<u32>();
                for i in 0..8 {
                    dst.add(i).write(src.add(i).read());
                }
            } else {
                let src = src.cast::<u8>();
                for i in 0..32 {
                    dst.add(i).write(src.add(i).read());
                }
            }
        }
    }

    /// Copies 32 bytes from `src` to `dst` in reverse order, byte by byte: the
    /// big-endian/little-endian conversion without the delegation.
    ///
    /// # Safety
    /// `src` must be readable and `dst` writable for 32 bytes; they must not overlap.
    #[cfg(not(target_arch = "riscv32"))]
    #[inline(always)]
    unsafe fn reversed_byte_copy(src: *const u8, dst: *mut u8) {
        unsafe {
            for i in 0..32 {
                dst.add(31 - i).write(src.add(i).read());
            }
        }
    }

    /// Writes the slot `src` as 32 big-endian bytes at `dst` (any alignment). With the
    /// `bytereverse_delegation` feature on RISC-V the slot is byte-reversed in place
    /// and copied out, so it must be one the caller no longer needs. Without
    /// delegation, RISC-V uses `rev8` on aligned destination words (shifted words
    /// and edge bytes for unaligned `dst`); other targets use a reversed byte copy.
    /// Both non-delegation paths leave the slot untouched.
    ///
    /// # Safety
    /// `src` must be 32 bytes aligned and initialized, `dst` must be writable for 32
    /// bytes; they must not overlap.
    #[inline(always)]
    pub unsafe fn write_slot_as_be_bytes(src: *mut Self, dst: *mut u8) {
        #[cfg(all(target_arch = "riscv32", feature = "bytereverse_delegation"))]
        unsafe {
            Self::bytereverse_in_place(src);
            Self::copy_slot_to_bytes(src, dst);
        }

        #[cfg(all(target_arch = "riscv32", not(feature = "bytereverse_delegation")))]
        unsafe {
            crate::byte_order::write_words_as_be_32(src.cast::<u32>(), dst);
        }

        #[cfg(not(target_arch = "riscv32"))]
        unsafe {
            Self::reversed_byte_copy(src.cast::<u8>(), dst);
        }
    }

    /// Writes the big-endian integer at `src` into the slot `dst`. With the
    /// `bytereverse_delegation` feature on RISC-V this is a plain copy and one
    /// in-place reversal delegation. Without delegation, RISC-V reads source
    /// words using `rev8` (shifting aligned words together if `src` is unaligned);
    /// other targets use a reversed byte-by-byte copy.
    ///
    /// # Safety
    /// `src` must be readable for 32 bytes, `dst` must be 32 bytes aligned and writable;
    /// source and destination must not overlap.
    #[inline(always)]
    pub unsafe fn write_be_bytes_into_slot(src: *const u8, dst: *mut Self) {
        #[cfg(all(target_arch = "riscv32", feature = "bytereverse_delegation"))]
        unsafe {
            Self::copy_bytes_into_slot(src, dst);
            Self::bytereverse_in_place(dst);
        }

        #[cfg(all(target_arch = "riscv32", not(feature = "bytereverse_delegation")))]
        unsafe {
            crate::byte_order::read_be_into_words::<32>(src, dst.cast::<u32>());
        }

        #[cfg(not(target_arch = "riscv32"))]
        unsafe {
            Self::reversed_byte_copy(src, dst.cast::<u8>());
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

    pub fn leading_zeros(&self) -> usize {
        let mut cnt = 0;

        for el in self.0.iter().rev() {
            if *el == 0 {
                cnt += 64
            } else {
                cnt += el.leading_zeros() as usize;
                return cnt;
            }
        }

        cnt
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
            write!(f, "{word:016x}")?;
        }

        core::fmt::Result::Ok(())
    }
}

#[cfg(test)]
mod byte_order_tests {
    use super::DelegatedU256;

    #[test]
    fn bytereverse_and_be_conversions_agree() {
        let mut be = [0u8; 32];
        for (i, b) in be.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(37).wrapping_add(11);
        }
        let value = DelegatedU256::from_be_bytes(&be);
        assert_eq!(value.to_be_bytes(), be);
        let mut out = [0u8; 32];
        value.write_be_bytes_into(&mut out);
        assert_eq!(out, be);
        let mut le = be;
        le.reverse();
        assert_eq!(*value.as_le_bytes(), le);
        assert_eq!(
            DelegatedU256::from_le_bytes(&le).as_limbs(),
            value.as_limbs()
        );
        let mut twice = value.clone();
        twice.bytereverse();
        twice.bytereverse();
        assert_eq!(twice.as_limbs(), value.as_limbs());
        // An unaligned source takes the byte path on the host and the glue path on riscv32.
        let mut buf = [0u8; 40];
        buf[3..35].copy_from_slice(&be);
        let unaligned: &[u8; 32] = buf[3..35].try_into().unwrap();
        assert_eq!(
            DelegatedU256::from_be_bytes(unaligned).as_limbs(),
            value.as_limbs()
        );
    }

    #[test]
    fn raw_be_conversions_handle_all_byte_alignments() {
        let be = core::array::from_fn::<_, 32, _>(|i| (i * 37 + 11) as u8);
        let expected = DelegatedU256::from_be_bytes(&be);

        for offset in 0..=3 {
            let mut source = [0u8; 36];
            source[offset..offset + 32].copy_from_slice(&be);
            let mut slot = DelegatedU256::default();
            // SAFETY: `source` contains 32 bytes from `offset`; `slot` is a
            // distinct, 32-aligned writable value.
            unsafe {
                DelegatedU256::write_be_bytes_into_slot(source.as_ptr().add(offset), &mut slot);
            }
            assert_eq!(slot.as_limbs(), expected.as_limbs());

            let mut output = [0xA5u8; 36];
            // SAFETY: `slot` is initialized and aligned; the separate output
            // array has 32 writable bytes from `offset`.
            unsafe {
                DelegatedU256::write_slot_as_be_bytes(&mut slot, output.as_mut_ptr().add(offset));
            }
            assert_eq!(&output[offset..offset + 32], &be);
            assert!(output[..offset].iter().all(|&byte| byte == 0xA5));
            assert!(output[offset + 32..].iter().all(|&byte| byte == 0xA5));
            #[cfg(not(all(target_arch = "riscv32", feature = "bytereverse_delegation")))]
            assert_eq!(slot.as_limbs(), expected.as_limbs());
        }

        let mut slot = expected.clone();
        let mut out = [0u8; 32];
        slot.bytereverse_and_write_le(&mut out);
        assert_eq!(out, be);
        #[cfg(not(all(target_arch = "riscv32", feature = "bytereverse_delegation")))]
        assert_eq!(slot.as_limbs(), expected.as_limbs());
    }
}

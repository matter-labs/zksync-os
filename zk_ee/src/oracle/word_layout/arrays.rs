use super::WordLayout;

/// Generic array impl for all WordLayout types.
///
/// - For `[u8; N]`: uses byte-packed encoding (4 bytes per word, last word zero-padded).
/// - For other types where the array's memory layout matches the word layout
///   (no padding, alignment >= 4): uses bulk u32 store loop.
/// - Otherwise: reads element-by-element.
impl<T: WordLayout, const N: usize> WordLayout for [T; N] {
    const WORD_COUNT: Option<usize> = if core::mem::size_of::<T>() == 1 {
        // Byte-packed: ceil(N/4) words
        Some(N.div_ceil(4))
    } else {
        match T::WORD_COUNT {
            Some(wc) => Some(wc * N),
            None => None,
        }
    };

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        if core::mem::size_of::<T>() == 1 {
            // Byte-packed path
            let bytes = unsafe { core::slice::from_raw_parts(self.as_ptr() as *const u8, N) };
            let mut i = 0;
            while i < N {
                let mut buf = [0u8; 4];
                let take = if N - i < 4 { N - i } else { 4 };
                buf[..take].copy_from_slice(&bytes[i..i + take]);
                w(u32::from_le_bytes(buf));
                i += 4;
            }
        } else {
            for val in self {
                val.write_words(w);
            }
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        if core::mem::size_of::<T>() == 1 {
            // Byte-packed path
            let mut result = core::mem::MaybeUninit::<Self>::uninit();
            let bytes =
                unsafe { core::slice::from_raw_parts_mut(result.as_mut_ptr() as *mut u8, N) };
            let mut i = 0;
            while i < N {
                let word = r().to_le_bytes();
                let take = if N - i < 4 { N - i } else { 4 };
                bytes[i..i + take].copy_from_slice(&word[..take]);
                i += 4;
            }
            unsafe { result.assume_init() }
        } else if core::mem::size_of::<Self>()
            == match Self::WORD_COUNT {
                Some(wc) => wc * 4,
                None => usize::MAX,
            }
            && core::mem::align_of::<Self>() >= 4
            && N > 0
        {
            // Bulk path: memory layout matches word layout
            let mut result = core::mem::MaybeUninit::<Self>::uninit();
            let dst = result.as_mut_ptr() as *mut u32;
            let word_count = core::mem::size_of::<Self>() / 4;
            for i in 0..word_count {
                unsafe {
                    dst.add(i).write(r());
                }
            }
            unsafe { result.assume_init() }
        } else {
            // Element-by-element
            core::array::from_fn(|_| T::read_words(r))
        }
    }
}

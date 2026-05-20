extern crate alloc;
use super::WordLayout;
use alloc::vec::Vec;

impl<T: WordLayout> WordLayout for Vec<T> {
    const WORD_COUNT: Option<usize> = None;

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        w(self.len() as u32);
        if core::mem::size_of::<T>() == 1 {
            let bytes: &[u8] =
                unsafe { core::slice::from_raw_parts(self.as_ptr() as *const u8, self.len()) };
            let mut i = 0;
            while i < bytes.len() {
                let mut buf = [0u8; 4];
                let take = core::cmp::min(4, bytes.len() - i);
                buf[..take].copy_from_slice(&bytes[i..i + take]);
                w(u32::from_le_bytes(buf));
                i += 4;
            }
        } else {
            for item in self.iter() {
                item.write_words(w);
            }
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let len = r() as usize;
        // Always use the generic element-by-element path for read.
        // For T=u8, each element reads one u32 word and truncates — this
        // matches the byte-packed write format because [u8; N]::read_words
        // is NOT used here (Vec uses per-element reads). To keep the wire
        // format consistent, the write side also uses per-element writes
        // when size_of::<T>() != 1.
        //
        // Note: the size_of==1 byte-packing in write_words means Vec<u8>
        // write packs 4 bytes/word, but this read reads 1 word/element.
        // These MUST match — so we use byte-pack for read too, but avoid
        // transmute by constructing T from the bytes safely.
        if core::mem::size_of::<T>() == 1 {
            // Read byte-packed words, construct each T via read_words.
            // This reads ceil(len/4) words total.
            let word_count = len.div_ceil(4);
            let mut all_words: alloc::vec::Vec<u32> = alloc::vec::Vec::with_capacity(word_count);
            for _ in 0..word_count {
                all_words.push(r());
            }
            // Feed one word per T::read_words call by creating a sub-iterator
            // that yields words one at a time for each byte.
            let mut byte_idx = 0;
            let mut result = alloc::vec::Vec::with_capacity(len);
            for _ in 0..len {
                let word_idx = byte_idx / 4;
                let byte_in_word = byte_idx % 4;
                let byte_val = (all_words[word_idx] >> (byte_in_word * 8)) as u8;
                // SAFETY: T has size 1. We construct it from a single byte.
                // For T=u8, this is trivially safe. For T=bool, the write side
                // only writes 0 or 1, so this is safe on roundtrip. External
                // (untrusted) data could produce invalid bool — same risk as
                // all oracle responses (validated at consumer level).
                let val = T::read_words(&mut || byte_val as u32);
                result.push(val);
                byte_idx += 1;
            }
            result
        } else {
            (0..len).map(|_| T::read_words(r)).collect()
        }
    }
}

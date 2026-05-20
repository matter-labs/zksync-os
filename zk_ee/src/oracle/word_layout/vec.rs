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
        if core::mem::size_of::<T>() == 1 {
            let mut bytes = alloc::vec![0u8; len];
            let mut i = 0;
            while i < len {
                let word = r().to_le_bytes();
                let take = core::cmp::min(4, len - i);
                bytes[i..i + take].copy_from_slice(&word[..take]);
                i += 4;
            }
            // SAFETY: Vec<u8> and Vec<T> have same layout when size_of::<T>() == 1
            unsafe { core::mem::transmute(bytes) }
        } else {
            (0..len).map(|_| T::read_words(r)).collect()
        }
    }
}

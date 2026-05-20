use crate::utils::usize_rw::num_usize_words_for_u8_capacity;

use super::USIZE_SIZE;
use core::{alloc::Allocator, mem::MaybeUninit};

pub fn allocate_vec_usize_aligned<A: Allocator>(
    byte_size: usize,
    allocator: A,
) -> alloc::vec::Vec<u8, A> {
    let usize_size = num_usize_words_for_u8_capacity(byte_size);
    let allocated: alloc::vec::Vec<usize, A> =
        alloc::vec::Vec::with_capacity_in(usize_size, allocator);

    let (ptr, len, capacity, allocator) = allocated.into_raw_parts_with_alloc();
    let new_capacity = capacity * USIZE_SIZE;
    let new_len = len * USIZE_SIZE;
    assert!(new_capacity >= byte_size);
    let new_ptr = ptr.cast::<u8>();

    unsafe { alloc::vec::Vec::from_raw_parts_in(new_ptr, new_len, new_capacity, allocator) }
}

// Clone preserves both the raw buffer bytes and initialization accounting.
#[derive(Clone)]
pub struct UsizeAlignedByteBox<A: Allocator> {
    inner: alloc::boxed::Box<[MaybeUninit<usize>], A>,
    byte_capacity: usize,
    initialized_bytes: usize,
}

impl<A: Allocator> core::fmt::Debug for UsizeAlignedByteBox<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("UsizeAlignedByteBox")
            .field("word_capacity", &self.inner.len())
            .field("byte_capacity", &self.byte_capacity)
            .field("initialized_bytes", &self.initialized_bytes)
            .finish()
    }
}

impl<A: Allocator> AsRef<[u8]> for UsizeAlignedByteBox<A> {
    fn as_ref(&self) -> &[u8] {
        Self::as_slice(self)
    }
}

impl<A: Allocator> UsizeAlignedByteBox<A> {
    pub fn preallocated_in(byte_capacity: usize, allocator: A) -> Self {
        let num_usize_words = num_usize_words_for_u8_capacity(byte_capacity);
        let inner: alloc::boxed::Box<[MaybeUninit<usize>], A> =
            alloc::boxed::Box::new_uninit_slice_in(num_usize_words, allocator);

        Self {
            inner,
            byte_capacity,
            initialized_bytes: 0,
        }
    }

    /// Raw pointer to the backing for direct writes.
    pub fn inner_mut_ptr(&mut self) -> *mut MaybeUninit<usize> {
        self.inner.as_mut_ptr()
    }

    /// Mark `n` bytes as initialized (after direct writes via inner_mut_ptr).
    pub fn mark_initialized(&mut self, n: usize) {
        self.initialized_bytes = n;
    }

    pub fn as_slice(&self) -> &[u8] {
        debug_assert!(self.inner.len() * USIZE_SIZE >= self.byte_capacity);
        assert!(
            self.initialized_bytes >= self.byte_capacity,
            "trying to access {} bytes, but only {} bytes are initialized",
            self.byte_capacity,
            self.initialized_bytes
        );
        unsafe { core::slice::from_raw_parts(self.inner.as_ptr().cast::<u8>(), self.byte_capacity) }
    }

    pub fn len(&self) -> usize {
        self.byte_capacity
    }

    pub fn from_slice_in(src: &[u8], allocator: A) -> Self {
        let mut result = Self::preallocated_in(src.len(), allocator);
        // copy
        unsafe {
            core::ptr::copy_nonoverlapping(
                src.as_ptr(),
                result.inner.as_mut_ptr().cast::<u8>(),
                src.len(),
            );
        }
        result.initialized_bytes = src.len();

        result
    }

    pub fn from_slices_in(srcs: &[&[u8]], allocator: A) -> Self {
        let total_len: usize = srcs.iter().map(|s| s.len()).sum();

        let mut result = Self::preallocated_in(total_len, allocator);

        unsafe {
            let mut dst = result.inner.as_mut_ptr().cast::<u8>();
            for src in srcs {
                core::ptr::copy_nonoverlapping(src.as_ptr(), dst, src.len());
                dst = dst.add(src.len());
            }
        }
        result.initialized_bytes = total_len;

        result
    }

    pub fn from_usize_iterator_in(src: impl ExactSizeIterator<Item = usize>, allocator: A) -> Self {
        let word_capacity = src.len();
        let mut inner: alloc::boxed::Box<[MaybeUninit<usize>], A> =
            alloc::boxed::Box::new_uninit_slice_in(word_capacity, allocator);
        // iterators will have same length by the contract
        unsafe {
            core::hint::assert_unchecked(src.len() == inner.len());
        }
        for (src, dst) in src.zip(inner.iter_mut()) {
            dst.write(src);
        }
        let byte_capacity = word_capacity * USIZE_SIZE;

        Self {
            inner,
            byte_capacity,
            initialized_bytes: byte_capacity,
        }
    }

    pub fn from_init_fn_in(
        buffer_size: usize,
        init_fn: impl FnOnce(&mut [MaybeUninit<usize>]) -> usize,
        allocator: A,
    ) -> Self {
        let mut inner: alloc::boxed::Box<[MaybeUninit<usize>], A> =
            alloc::boxed::Box::new_uninit_slice_in(buffer_size, allocator);
        let written_words = init_fn(&mut inner);
        assert!(written_words <= buffer_size); // we do not want to truncate or realloc, but we will expose only written part below
                                               // Safety: init_fn only guarantees that it initialized `written_words` elements.
                                               // Initialize the remainder to keep the full allocation initialized.
        for dst in inner.iter_mut().skip(written_words) {
            dst.write(0);
        }
        let byte_capacity = written_words * USIZE_SIZE; // we only count initialized words for capacity purposes

        Self {
            inner,
            byte_capacity,
            initialized_bytes: buffer_size * USIZE_SIZE,
        }
    }

    #[track_caller]
    pub fn truncated_to_byte_length(&mut self, byte_len: usize) {
        assert!(
            byte_len <= self.byte_capacity,
            "trying to truncate to {} bytes, while capacity is just {} bytes",
            byte_len,
            self.byte_capacity
        );
        self.byte_capacity = byte_len;
    }
}

#[cfg(test)]
mod tests {
    use std::panic::AssertUnwindSafe;

    use std::alloc::Global;

    use super::{allocate_vec_usize_aligned, UsizeAlignedByteBox, USIZE_SIZE};
    use crate::utils::usize_rw::num_usize_words_for_u8_capacity;

    #[test]
    fn num_usize_words_for_u8_capacity_rounds_up_and_keeps_even_word_count() {
        assert_eq!(num_usize_words_for_u8_capacity(0), 0);
        assert_eq!(num_usize_words_for_u8_capacity(1), 2);
        assert_eq!(num_usize_words_for_u8_capacity(USIZE_SIZE), 2);
        assert_eq!(num_usize_words_for_u8_capacity(USIZE_SIZE + 1), 2);
        assert_eq!(num_usize_words_for_u8_capacity(2 * USIZE_SIZE), 2);
        assert_eq!(num_usize_words_for_u8_capacity(2 * USIZE_SIZE + 1), 4);
    }

    #[test]
    fn allocate_vec_usize_aligned_has_aligned_capacity() {
        let requested = USIZE_SIZE + 1;
        let buffer = allocate_vec_usize_aligned(requested, Global);

        assert_eq!(buffer.len(), 0);
        assert!(buffer.capacity() >= requested);
        assert_eq!(buffer.capacity() % USIZE_SIZE, 0);
    }

    #[test]
    fn preallocated_len_reports_requested_byte_length() {
        let requested = USIZE_SIZE + 3;
        let buffer = UsizeAlignedByteBox::preallocated_in(requested, Global);

        assert_eq!(buffer.len(), requested);
    }

    #[test]
    fn preallocated_panics_if_read_before_init() {
        let buffer = UsizeAlignedByteBox::preallocated_in(1, Global);

        let panicked = std::panic::catch_unwind(|| {
            let _ = buffer.as_slice();
        })
        .is_err();

        assert!(panicked);
    }

    #[test]
    fn from_slice_in_roundtrip_and_as_ref() {
        let input = [1u8, 2, 3, 4, 5];
        let buffer = UsizeAlignedByteBox::from_slice_in(&input, Global);

        assert_eq!(buffer.len(), input.len());
        assert_eq!(buffer.as_slice(), &input);
        assert_eq!(buffer.as_ref(), &input);
    }

    #[test]
    fn from_slices_in_roundtrip() {
        let a = [1u8, 2];
        let b = [];
        let c = [3u8, 4, 5];
        let buffer = UsizeAlignedByteBox::from_slices_in(&[&a, &b, &c], Global);

        assert_eq!(buffer.len(), a.len() + b.len() + c.len());
        assert_eq!(buffer.as_slice(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn from_slices_in_empty_input() {
        let srcs: [&[u8]; 0] = [];
        let buffer = UsizeAlignedByteBox::from_slices_in(&srcs, Global);

        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.as_slice(), &[]);
    }

    #[test]
    fn from_usize_iterator_in_serializes_words() {
        let words = [1usize, 2usize, usize::MAX];
        let expected: alloc::vec::Vec<u8> =
            words.iter().flat_map(|word| word.to_ne_bytes()).collect();
        let buffer = UsizeAlignedByteBox::from_usize_iterator_in(words.into_iter(), Global);

        assert_eq!(buffer.len(), words.len() * USIZE_SIZE);
        assert_eq!(buffer.as_slice(), expected.as_slice());
    }

    #[test]
    fn from_init_fn_in_uses_written_word_count_for_len() {
        let buffer = UsizeAlignedByteBox::from_init_fn_in(
            4,
            |dst| {
                dst[0].write(11usize);
                dst[1].write(22usize);
                2
            },
            Global,
        );

        let expected: alloc::vec::Vec<u8> = [11usize, 22usize]
            .into_iter()
            .flat_map(|word| word.to_ne_bytes())
            .collect();
        assert_eq!(buffer.len(), 2 * USIZE_SIZE);
        assert_eq!(buffer.as_slice(), expected.as_slice());
    }

    #[test]
    fn from_init_fn_in_panics_if_written_words_exceed_buffer_size() {
        let panicked = std::panic::catch_unwind(|| {
            UsizeAlignedByteBox::from_init_fn_in(1, |_dst| 2, Global);
        })
        .is_err();

        assert!(panicked);
    }

    #[test]
    fn truncated_to_byte_length_reduces_visible_len() {
        let mut buffer = UsizeAlignedByteBox::from_slice_in(&[1, 2, 3, 4], Global);
        buffer.truncated_to_byte_length(3);

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn truncated_to_byte_length_panics_if_new_len_is_too_large() {
        let mut buffer = UsizeAlignedByteBox::from_slice_in(&[1, 2, 3], Global);
        let panicked = std::panic::catch_unwind(AssertUnwindSafe(|| {
            buffer.truncated_to_byte_length(4);
        }))
        .is_err();

        assert!(panicked);
    }
}

/// WordLayout for UsizeAlignedByteBox: same wire format as Vec<u8>
/// (u32 byte length + byte-packed u32 words). Reads directly into the
/// usize-aligned backing — one allocation, zero copies.
impl<A: Allocator + Default> crate::oracle::word_layout::WordLayout for UsizeAlignedByteBox<A> {
    const WORD_COUNT: Option<usize> = None;

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        (self.byte_capacity as u32).write_words(w);
        let bytes = self.as_slice();
        let mut i = 0;
        while i < bytes.len() {
            let mut buf = [0u8; 4];
            let take = core::cmp::min(4, bytes.len() - i);
            buf[..take].copy_from_slice(&bytes[i..i + take]);
            w(u32::from_le_bytes(buf));
            i += 4;
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let byte_len = u32::read_words(r) as usize;
        let u32_word_count = byte_len.div_ceil(4);
        let mut result = Self::preallocated_in(byte_len, A::default());
        let dst = result.inner_mut_ptr();
        cfg_if::cfg_if! {
            if #[cfg(target_pointer_width = "32")] {
                // usize = u32: direct 1:1 mapping
                for i in 0..u32_word_count {
                    unsafe { (dst as *mut u32).add(i).write(r()); }
                }
            } else {
                // usize = u64: pack two u32 words per usize
                let mut i = 0;
                while i < u32_word_count {
                    let lo = r() as u64;
                    let hi = if i + 1 < u32_word_count { r() as u64 } else { 0 };
                    unsafe { (dst as *mut u64).add(i / 2).write(lo | (hi << 32)); }
                    i += 2;
                }
            }
        }
        result.mark_initialized(byte_len);
        result
    }

    fn read_words_into(&mut self, r: &mut impl FnMut() -> u32) {
        let byte_len = r() as usize;
        let u32_word_count = byte_len.div_ceil(4);
        let needed_usize_words = num_usize_words_for_u8_capacity(byte_len);
        if self.inner.len() < needed_usize_words {
            *self = Self::read_words(&mut || {
                // Replay: we already consumed the length word, so prepend it
                unreachable!("pre-allocated buffer should be large enough")
            });
            return;
        }
        self.byte_capacity = byte_len;
        let dst = self.inner_mut_ptr();
        cfg_if::cfg_if! {
            if #[cfg(target_pointer_width = "32")] {
                for i in 0..u32_word_count {
                    unsafe { (dst as *mut u32).add(i).write(r()); }
                }
            } else {
                let mut i = 0;
                while i < u32_word_count {
                    let lo = r() as u64;
                    let hi = if i + 1 < u32_word_count { r() as u64 } else { 0 };
                    unsafe { (dst as *mut u64).add(i / 2).write(lo | (hi << 32)); }
                    i += 2;
                }
            }
        }
        self.mark_initialized(byte_len);
    }
}

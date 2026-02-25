use crate::utils::usize_rw::{AsUsizeWritable, SafeUsizeWritable, UsizeWriteable};

use super::USIZE_SIZE;
use core::{alloc::Allocator, mem::MaybeUninit};

pub const fn num_usize_words_for_u8_capacity(u8_capacity: usize) -> usize {
    let num_words = u8_capacity.div_ceil(USIZE_SIZE);
    // give it some slack to account for 64/32 bit architectures mismatch
    num_words.next_multiple_of(2)
}

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
    // Number of initialized bytes in `inner`.
    // Constructors that copy byte slices track this precisely (byte-granular),
    // while `UsizeSliceWriter` advances this in whole words on drop.
    // The value is monotonic and must be >= `byte_capacity` before `as_slice`.
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

impl<A: Allocator> AsUsizeWritable for UsizeAlignedByteBox<A> {
    type Writable<'a>
        = UsizeSliceWriter<'a>
    where
        Self: 'a;
    fn as_writable<'a>(&'a mut self) -> Self::Writable<'a>
    where
        Self: 'a,
    {
        let range = self.inner.as_mut_ptr_range();

        UsizeSliceWriter {
            start: range.start,
            dst: range.start,
            end: range.end,
            initialized_bytes: &mut self.initialized_bytes,
            _marker: core::marker::PhantomData,
        }
    }
}

pub struct UsizeSliceWriter<'a> {
    start: *mut MaybeUninit<usize>,
    dst: *mut MaybeUninit<usize>,
    end: *mut MaybeUninit<usize>,
    initialized_bytes: &'a mut usize,
    _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> UsizeWriteable for UsizeSliceWriter<'a> {
    unsafe fn write_usize(&mut self, value: usize) {
        self.dst.write(MaybeUninit::new(value));
        self.dst = self.dst.add(1);
    }
}

impl<'a> SafeUsizeWritable for UsizeSliceWriter<'a> {
    fn try_write(&mut self, value: usize) -> Result<(), ()> {
        if self.dst >= self.end {
            Err(())
        } else {
            unsafe { self.write_usize(value) };

            Ok(())
        }
    }

    fn len(&self) -> usize {
        unsafe { self.end.offset_from_unsigned(self.dst) }
    }
}

impl Drop for UsizeSliceWriter<'_> {
    fn drop(&mut self) {
        // Track how many words this writer advanced from the beginning of the allocation.
        // The tx path writes from offset 0, so this reflects the initialized prefix in bytes.
        let words_written = unsafe { self.dst.offset_from_unsigned(self.start) };
        let bytes_written = words_written
            .checked_mul(USIZE_SIZE)
            .expect("bytes written must fit in usize");
        // Keep monotonic initialization tracking in case multiple writers are created.
        if *self.initialized_bytes < bytes_written {
            *self.initialized_bytes = bytes_written;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::panic::AssertUnwindSafe;

    use std::alloc::Global;

    use super::{
        allocate_vec_usize_aligned, num_usize_words_for_u8_capacity, UsizeAlignedByteBox,
        USIZE_SIZE,
    };
    use crate::utils::usize_rw::{AsUsizeWritable, SafeUsizeWritable, UsizeWriteable};

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
    fn writer_drop_without_writes_keeps_existing_initialized_prefix() {
        let input = [1u8, 2, 3, 4, 5];
        let mut buffer = UsizeAlignedByteBox::from_slice_in(&input, Global);

        {
            let _writer = buffer.as_writable();
            // Intentionally perform no writes.
        }

        assert_eq!(buffer.as_slice(), &input);
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

    #[test]
    fn usize_slice_writer_try_write_updates_len_and_writes_data() {
        let byte_len = 2 * USIZE_SIZE;
        let mut buffer = UsizeAlignedByteBox::preallocated_in(byte_len, Global);

        {
            let mut writer = buffer.as_writable();
            assert_eq!(writer.len(), num_usize_words_for_u8_capacity(byte_len));
            writer.try_write(111).unwrap();
            assert_eq!(writer.len(), 1);
            writer.try_write(222).unwrap();
            assert_eq!(writer.len(), 0);
        }

        let expected: alloc::vec::Vec<u8> = [111usize, 222usize]
            .into_iter()
            .flat_map(|word| word.to_ne_bytes())
            .collect();
        assert_eq!(buffer.as_slice(), expected.as_slice());
    }

    #[test]
    fn usize_slice_writer_try_write_returns_err_when_out_of_bounds() {
        let mut buffer = UsizeAlignedByteBox::preallocated_in(0, Global);

        let mut writer = buffer.as_writable();
        assert_eq!(writer.len(), 0);
        assert!(writer.try_write(1).is_err());
    }

    #[test]
    fn usize_slice_writer_unsafe_write_usize_path_writes_data() {
        let byte_len = USIZE_SIZE + 1;
        let mut buffer = UsizeAlignedByteBox::preallocated_in(byte_len, Global);

        {
            let mut writer = buffer.as_writable();
            unsafe {
                UsizeWriteable::write_usize(&mut writer, usize::MAX);
                UsizeWriteable::write_usize(&mut writer, 0);
            }
        }

        let mut expected: alloc::vec::Vec<u8> = usize::MAX.to_ne_bytes().to_vec();
        expected.extend([0usize; 1].iter().flat_map(|word| word.to_ne_bytes()));
        expected.truncate(byte_len);
        assert_eq!(buffer.as_slice().len(), byte_len);
        assert_eq!(buffer.as_slice(), expected.as_slice());
    }

    #[test]
    fn preallocated_partial_write_panics_on_read() {
        let mut buffer = UsizeAlignedByteBox::preallocated_in(2 * USIZE_SIZE, Global);

        {
            let mut writer = buffer.as_writable();
            writer.try_write(123).unwrap();
            // One word initialized, but `byte_capacity` requires two words.
        }

        let panicked = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = buffer.as_slice();
        }))
        .is_err();
        assert!(panicked);
    }
}

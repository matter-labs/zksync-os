use super::{
    copy_bytes_iter_to_usize_slice,
    usize_rw::{AsUsizeWritable, SafeUsizeWritable, UsizeWriteable},
    USIZE_SIZE,
};
use core::alloc::Allocator;

pub const fn num_usize_words_for_u8_capacity(u8_capacity: usize) -> usize {
    let num_words = u8_capacity.next_multiple_of(USIZE_SIZE) / USIZE_SIZE;
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

#[derive(Clone, Debug)]
pub struct UsizeAlignedByteBox<A: Allocator> {
    inner: alloc::boxed::Box<[usize], A>,
    byte_capacity: usize,
}

impl<A: Allocator> UsizeAlignedByteBox<A> {
    pub fn preallocated_in(byte_capacity: usize, allocator: A) -> Self {
        let num_usize_words = num_usize_words_for_u8_capacity(byte_capacity);
        let inner: alloc::boxed::Box<[usize], A> = unsafe {
            alloc::boxed::Box::new_uninit_slice_in(num_usize_words, allocator).assume_init()
        };

        Self {
            inner,
            byte_capacity,
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        debug_assert!(self.inner.len() * USIZE_SIZE >= self.byte_capacity);
        unsafe { core::slice::from_raw_parts(self.inner.as_ptr().cast::<u8>(), self.byte_capacity) }
    }

    pub fn len(&self) -> usize {
        self.byte_capacity
    }

    pub fn from_u8_iterator_in(src: impl ExactSizeIterator<Item = u8>, allocator: A) -> Self {
        let mut result = Self::preallocated_in(src.len(), allocator);
        copy_bytes_iter_to_usize_slice(src, &mut result.inner);

        result
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

        result
    }

    pub fn from_usize_iterator_in(src: impl ExactSizeIterator<Item = usize>, allocator: A) -> Self {
        let mut inner: alloc::boxed::Box<[usize], A> =
            unsafe { alloc::boxed::Box::new_uninit_slice_in(src.len(), allocator).assume_init() };
        let mut dst = inner.as_mut_ptr();
        for word in src {
            unsafe {
                dst.write(word);
                dst = dst.add(1);
            }
        }

        let byte_capacity = inner.len() * USIZE_SIZE;

        Self {
            inner,
            byte_capacity,
        }
    }

    pub fn truncated_to_byte_length(&mut self, byte_len: usize) {
        assert!(byte_len <= self.byte_capacity);
        self.byte_capacity = byte_len;
    }

    pub fn into_pinned(self) -> UsizeAlignedPinnedByteBox<A>
    where
        A: 'static,
    {
        let Self {
            inner,
            byte_capacity,
        } = self;

        UsizeAlignedPinnedByteBox {
            inner: alloc::boxed::Box::into_pin(inner),
            byte_capacity,
        }
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
            dst: range.start,
            end: range.end,
            _marker: core::marker::PhantomData,
        }
    }
}

pub struct UsizeSliceWriter<'a> {
    dst: *mut usize,
    end: *mut usize,
    _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> UsizeWriteable for UsizeSliceWriter<'a> {
    unsafe fn write_usize(&mut self, value: usize) {
        self.dst.write(value);
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

#[derive(Clone, Debug)]
pub struct UsizeAlignedPinnedByteBox<A: Allocator> {
    inner: core::pin::Pin<alloc::boxed::Box<[usize], A>>,
    byte_capacity: usize,
}

impl<A: Allocator> UsizeAlignedPinnedByteBox<A> {
    pub fn as_slice(&self) -> &[u8] {
        debug_assert!(self.inner.len() * USIZE_SIZE >= self.byte_capacity);
        unsafe { core::slice::from_raw_parts(self.inner.as_ptr().cast::<u8>(), self.byte_capacity) }
    }

    pub fn len(&self) -> usize {
        self.byte_capacity
    }
}

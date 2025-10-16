use crate::utils::usize_rw::UsizeWriteable;

use super::*;

pub struct AlignedBuffer<'a, const N: usize>
where
    Assert<{ is_proper_buffer_alignment(N) }>: IsTrue,
{
    inner: &'a mut [u8],
}

pub const fn is_proper_buffer_alignment(alignment: usize) -> bool {
    alignment.is_power_of_two() && alignment >= core::mem::size_of::<usize>()
}

impl<'a, const N: usize> AlignedBuffer<'a, N>
where
    Assert<{ is_proper_buffer_alignment(N) }>: IsTrue,
{
    pub fn new(dst: &'a mut [u8]) -> Self {
        assert!(dst.as_ptr().is_aligned_to(N));
        assert!(dst.len() % N == 0);

        Self { inner: dst }
    }
}

impl<'a, const N: usize> AsRef<[u8]> for AlignedBuffer<'a, N>
where
    Assert<{ is_proper_buffer_alignment(N) }>: IsTrue,
{
    fn as_ref(&self) -> &[u8] {
        &*self.inner
    }
}

impl<'a, const N: usize> AsMut<[u8]> for AlignedBuffer<'a, N>
where
    Assert<{ is_proper_buffer_alignment(N) }>: IsTrue,
{
    fn as_mut(&mut self) -> &mut [u8] {
        self.inner
    }
}

pub const USIZE_ALIGNMENT: usize = core::mem::align_of::<usize>();
pub const USIZE_SIZE: usize = core::mem::size_of::<usize>();
pub const U64_SIZE: usize = core::mem::size_of::<u64>();
pub const U64_ALIGNMENT: usize = core::mem::align_of::<u64>();

const _: () = const {
    assert!(U64_ALIGNMENT >= USIZE_ALIGNMENT);
    assert!(U64_SIZE >= USIZE_SIZE);
};

pub type U64AlignedBuffer<'a> = AlignedBuffer<'a, U64_ALIGNMENT>;

pub struct AlignedBufferIterator<'a> {
    pub(crate) start: *const usize,
    pub(crate) end: *const usize,
    pub(crate) _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> U64AlignedBuffer<'a> {
    pub const fn len_in_usize(&self) -> usize {
        debug_assert!(self.inner.len() % USIZE_SIZE == 0);
        self.inner.len() / USIZE_SIZE
    }

    pub fn iter(&self) -> AlignedBufferIterator<'_> {
        debug_assert!(self.inner.len() % USIZE_SIZE == 0);
        debug_assert!(self.inner.as_ptr().is_aligned_to(USIZE_SIZE));
        let start = self.inner.as_ptr().cast();
        let end = self.inner.as_ptr_range().end.cast();
        // by constructor and checks we can cast the pointer to usize
        AlignedBufferIterator {
            start,
            end,
            _marker: core::marker::PhantomData,
        }
    }

    pub fn as_usize_slice(&self) -> &[usize] {
        let (ptr, len) = (self.inner.as_ptr(), self.inner.len());
        let ptr = ptr.cast();
        let len = len / USIZE_SIZE;

        unsafe { core::slice::from_raw_parts(ptr, len) }
    }

    pub fn as_usize_slice_mut(&mut self) -> &mut [usize] {
        let (ptr, len) = (self.inner.as_mut_ptr(), self.inner.len());
        let ptr = ptr.cast();
        let len = len / USIZE_SIZE;

        unsafe { core::slice::from_raw_parts_mut(ptr, len) }
    }
}

impl<'a> AlignedBufferIterator<'a> {
    pub const fn empty() -> Self {
        Self {
            start: core::ptr::null(),
            end: core::ptr::null(),
            _marker: core::marker::PhantomData,
        }
    }
}

impl<'a> Iterator for AlignedBufferIterator<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        if self.start < self.end {
            unsafe {
                let item = self.start.read();
                self.start = self.start.add(1);

                Some(item)
            }
        } else {
            None
        }
    }
}

impl<'a> ExactSizeIterator for AlignedBufferIterator<'a> {
    fn len(&self) -> usize {
        if self.start >= self.end {
            0
        } else {
            unsafe { self.end.offset_from_unsigned(self.start) }
        }
    }
}

pub fn copy_bytes_to_usize_buffer(src: &[u8], dst: &mut impl UsizeWriteable) -> usize {
    let mut it = src.array_chunks::<USIZE_SIZE>();
    let mut written = it.len();
    for src in &mut it {
        unsafe {
            dst.write_usize(usize::from_le_bytes(*src));
        }
    }
    let remainder = it.remainder();
    if !remainder.is_empty() {
        written += 1;
        let mut buffer = 0usize.to_le_bytes();
        buffer[..remainder.len()].copy_from_slice(remainder);
        unsafe {
            dst.write_usize(usize::from_le_bytes(buffer));
        }
    }

    written
}

pub fn copy_bytes_iter_to_usize_buffer(
    src: impl ExactSizeIterator<Item = u8>,
    dst: &mut impl UsizeWriteable,
) -> usize {
    let mut it = src.array_chunks::<USIZE_SIZE>();
    let mut written = it.len();
    for src in &mut it {
        unsafe {
            dst.write_usize(usize::from_ne_bytes(src));
        }
    }
    if let Some(remainder) = it.into_remainder() {
        if remainder.len() > 0 {
            written += 1;
            let mut buffer = 0usize.to_ne_bytes();
            for (dst, src) in buffer.iter_mut().zip(remainder) {
                *dst = src;
            }
            unsafe {
                dst.write_usize(usize::from_le_bytes(buffer));
            }
        }
    }

    written
}

pub fn copy_bytes_iter_to_usize_slice(
    src: impl ExactSizeIterator<Item = u8>,
    dst: &mut [usize],
) -> usize {
    let min_capacity = num_usize_words_for_u8_capacity(src.len());
    assert!(dst.len() >= min_capacity);
    let mut it = src.array_chunks::<USIZE_SIZE>();
    let mut written = it.len();
    // go unsafe
    let mut dst = dst.as_mut_ptr();
    for src in &mut it {
        unsafe {
            dst.write(usize::from_ne_bytes(src));
            dst = dst.add(1);
        }
    }
    if let Some(remainder) = it.into_remainder() {
        if remainder.len() > 0 {
            written += 1;
            let mut buffer = 0usize.to_ne_bytes();
            for (dst, src) in buffer.iter_mut().zip(remainder) {
                *dst = src;
            }
            unsafe {
                dst.write(usize::from_ne_bytes(buffer));
            }
        }
    }

    written
}

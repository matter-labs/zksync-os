use core::alloc::Allocator;

mod trivial_impl;

// We should not make hard assumptions about how stack or heap is implemented
pub trait HostHeap {
    fn num_pages(&self) -> u32;
    #[allow(clippy::result_unit_err)]
    fn copy_into_memory(&mut self, src: &[u8], offset: u32) -> Result<(), ()>;
    #[allow(clippy::result_unit_err)]
    fn mem_read_into_buffer<const N: usize>(
        &self,
        dst: &mut [u8; N],
        offset: u32,
        num_bytes: u32,
    ) -> Result<(), ()> {
        Self::mem_read_into_slice(self, &mut dst[..(num_bytes as usize)], offset)
    }
    #[allow(clippy::result_unit_err)]
    fn mem_read_into_slice(&self, dst: &mut [u8], offset: u32) -> Result<(), ()>;
    #[allow(clippy::result_unit_err)]
    fn fill_memory(&mut self, byte: u8, offset: u32, len: u32) -> Result<(), ()>;
    #[allow(clippy::result_unit_err)]
    fn copy_memory(&mut self, src_offset: u32, dst_offset: u32, len: u32) -> Result<(), ()>;
}

pub trait ContinuousIndexAccess<T: Sized> {
    fn is_empty(&self) -> bool {
        ContinuousIndexAccess::len(self) == 0
    }
    fn len(&self) -> usize;
    fn get(&self, index: usize) -> Option<&T>;
    /// # Safety
    ///
    /// TODO: add docs
    unsafe fn get_unchecked(&self, index: usize) -> &T;
    /// # Safety
    ///
    /// TODO: add docs
    unsafe fn last_unchecked(&self) -> &T;
    /// # Safety
    ///
    /// TODO: add docs
    unsafe fn get_slice_unchecked(&self, range: core::ops::Range<usize>) -> &[T];
    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T>
    where
        Self: 'a,
        T: 'a;
}

impl<T: Sized> ContinuousIndexAccess<T> for [T] {
    fn is_empty(&self) -> bool {
        <[T]>::is_empty(self)
    }
    fn len(&self) -> usize {
        <[T]>::len(self)
    }
    fn get(&self, index: usize) -> Option<&T> {
        <[T]>::get(self, index)
    }
    unsafe fn get_unchecked(&self, index: usize) -> &T {
        <[T]>::get_unchecked(self, index)
    }
    unsafe fn last_unchecked(&self) -> &T {
        let idx = self.len() - 1;
        <[T]>::get_unchecked(self, idx)
    }
    unsafe fn get_slice_unchecked(&self, range: core::ops::Range<usize>) -> &[T] {
        &self[range]
    }
    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T>
    where
        Self: 'a,
        T: 'a,
    {
        <[T]>::iter(self)
    }
}

impl<T: Sized> ContinuousIndexAccess<T> for &[T] {
    fn is_empty(&self) -> bool {
        <[T]>::is_empty(self)
    }
    fn len(&self) -> usize {
        <[T]>::len(self)
    }
    fn get(&self, index: usize) -> Option<&T> {
        <[T]>::get(self, index)
    }
    unsafe fn get_unchecked(&self, index: usize) -> &T {
        <[T]>::get_unchecked(self, index)
    }
    unsafe fn last_unchecked(&self) -> &T {
        let idx = self.len() - 1;
        <[T]>::get_unchecked(self, idx)
    }
    unsafe fn get_slice_unchecked(&self, range: core::ops::Range<usize>) -> &[T] {
        &self[range]
    }
    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T>
    where
        Self: 'a,
        T: 'a,
    {
        <[T]>::iter(self)
    }
}

impl<T: Sized> ContinuousIndexAccess<T> for &mut [T] {
    fn is_empty(&self) -> bool {
        <[T]>::is_empty(self)
    }
    fn len(&self) -> usize {
        <[T]>::len(self)
    }
    fn get(&self, index: usize) -> Option<&T> {
        <[T]>::get(self, index)
    }
    unsafe fn get_unchecked(&self, index: usize) -> &T {
        <[T]>::get_unchecked(self, index)
    }
    unsafe fn last_unchecked(&self) -> &T {
        let idx = self.len() - 1;
        <[T]>::get_unchecked(self, idx)
    }
    unsafe fn get_slice_unchecked(&self, range: core::ops::Range<usize>) -> &[T] {
        &self[range]
    }
    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T>
    where
        Self: 'a,
        T: 'a,
    {
        <[T]>::iter(self)
    }
}

impl<T: Sized, V: ContinuousIndexAccess<T>> ContinuousIndexAccess<T> for &V {
    fn len(&self) -> usize {
        ContinuousIndexAccess::len(*self)
    }
    fn get(&self, index: usize) -> Option<&T> {
        ContinuousIndexAccess::get(*self, index)
    }
    unsafe fn get_unchecked(&self, index: usize) -> &T {
        ContinuousIndexAccess::get_unchecked(*self, index)
    }
    unsafe fn last_unchecked(&self) -> &T {
        ContinuousIndexAccess::last_unchecked(*self)
    }
    unsafe fn get_slice_unchecked(&self, range: core::ops::Range<usize>) -> &[T] {
        ContinuousIndexAccess::get_slice_unchecked(*self, range)
    }
    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T>
    where
        Self: 'a,
        T: 'a,
    {
        ContinuousIndexAccess::iter(*self)
    }
}

impl<T: Sized, V: ContinuousIndexAccess<T>> ContinuousIndexAccess<T> for &mut V {
    fn len(&self) -> usize {
        ContinuousIndexAccess::len(*self)
    }
    fn get(&self, index: usize) -> Option<&T> {
        ContinuousIndexAccess::get(*self, index)
    }
    unsafe fn get_unchecked(&self, index: usize) -> &T {
        ContinuousIndexAccess::get_unchecked(*self, index)
    }
    unsafe fn last_unchecked(&self) -> &T {
        ContinuousIndexAccess::last_unchecked(*self)
    }
    unsafe fn get_slice_unchecked(&self, range: core::ops::Range<usize>) -> &[T] {
        ContinuousIndexAccess::get_slice_unchecked(*self, range)
    }
    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T>
    where
        Self: 'a,
        T: 'a,
    {
        ContinuousIndexAccess::iter(*self)
    }
}

pub trait ContinuousIndexAccessMut<T: Sized>: ContinuousIndexAccess<T> {
    /// # Safety
    ///
    /// TODO: add docs
    unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T;
    /// # Safety
    ///
    /// TODO: add docs
    unsafe fn last_unchecked_mut(&mut self) -> &mut T;
    /// # Safety
    ///
    /// TODO: add docs
    unsafe fn get_slice_unchecked_mut(&mut self, range: core::ops::Range<usize>) -> &mut [T];
}

impl<T: Sized> ContinuousIndexAccessMut<T> for [T] {
    unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        <[T]>::get_unchecked_mut(self, index)
    }
    unsafe fn last_unchecked_mut(&mut self) -> &mut T {
        let len = self.len();
        <[T]>::get_unchecked_mut(self, len - 1)
    }
    unsafe fn get_slice_unchecked_mut(&mut self, range: core::ops::Range<usize>) -> &mut [T] {
        &mut self[range]
    }
}

impl<T: Sized> ContinuousIndexAccessMut<T> for &mut [T] {
    unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        <[T]>::get_unchecked_mut(self, index)
    }
    unsafe fn last_unchecked_mut(&mut self) -> &mut T {
        let len = self.len();
        <[T]>::get_unchecked_mut(self, len - 1)
    }
    unsafe fn get_slice_unchecked_mut(&mut self, range: core::ops::Range<usize>) -> &mut [T] {
        &mut self[range]
    }
}

impl<T: Sized, V: ContinuousIndexAccessMut<T>> ContinuousIndexAccessMut<T> for &mut V {
    unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        ContinuousIndexAccessMut::get_unchecked_mut(*self, index)
    }
    unsafe fn last_unchecked_mut(&mut self) -> &mut T {
        ContinuousIndexAccessMut::last_unchecked_mut(*self)
    }
    unsafe fn get_slice_unchecked_mut(&mut self, range: core::ops::Range<usize>) -> &mut [T] {
        ContinuousIndexAccessMut::get_slice_unchecked_mut(*self, range)
    }
}

pub trait ScratchSpaceRef<'this, T: Sized + 'this>: ContinuousIndexAccess<T> {}

pub trait ScratchSpaceCopiableRef<'this, T: Sized + 'this>:
    ScratchSpaceRef<'this, T> + Clone + Copy
{
}

pub trait ScratchSpaceRefMut<'this, T: Sized + 'this>:
    ScratchSpaceRef<'this, T> + ContinuousIndexAccessMut<T>
{
}

pub trait ScratchSpace<T: Sized>: ContinuousIndexAccessMut<T> {
    type Ref<'a>: ScratchSpaceCopiableRef<'a, T>
    where
        T: 'a,
        Self: 'a;
    type RefMut<'a>: ScratchSpaceRefMut<'a, T>
    where
        T: 'a,
        Self: 'a;

    fn by_ref(&self) -> Self::Ref<'_>;
    fn by_mut_ref(&mut self) -> Self::RefMut<'_>;

    fn clear(&mut self);
    fn truncate(&mut self, len: usize);
    #[allow(clippy::result_unit_err)]
    fn push(&mut self, el: T) -> Result<(), ()>;
    #[allow(clippy::result_unit_err)]
    fn put_many(&mut self, el: T, num_elements: usize) -> Result<(), ()>
    where
        T: Clone;
    fn pop(&mut self) -> Option<T>;
    fn drain_all(&mut self) -> impl Iterator<Item = T>;
}

pub trait OutputBuffer<T: Sized>: ContinuousIndexAccessMut<T> {
    type Ref<'a>: ScratchSpaceCopiableRef<'a, T>
    where
        T: 'a,
        Self: 'a;

    fn by_ref(&self) -> Self::Ref<'_>;
    #[allow(clippy::result_unit_err)]
    fn push(&mut self, el: T) -> Result<(), ()>;
}

pub trait SystemMemoryManager {
    type Allocator: Allocator + Clone;

    // owned cases
    type ScratchSpace<T: Sized>: ScratchSpace<T>;
    type OutputBuffer<T: Sized>: OutputBuffer<T>;

    fn empty_scratch_space<T: Sized>(&self) -> Self::ScratchSpace<T>;
    fn get_allocator(&self) -> Self::Allocator;
    #[allow(clippy::result_unit_err)]
    fn allocate_scratch_space<T: Sized>(
        &mut self,
        capacity: usize,
    ) -> Result<Self::ScratchSpace<T>, ()>;
    #[allow(clippy::result_unit_err)]
    fn allocate_output_buffer<T: Sized>(
        &mut self,
        capacity: usize,
    ) -> Result<Self::OutputBuffer<T>, ()>;
    #[allow(clippy::result_unit_err)]
    fn clone_scratch_space<T: Clone>(
        &mut self,
        existing_scratch: &Self::ScratchSpace<T>,
    ) -> Result<Self::ScratchSpace<T>, ()>;
}

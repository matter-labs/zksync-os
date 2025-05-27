use alloc::alloc::Global;
use alloc::vec::Vec;

use super::*;
use crate::constants::PAGE_SIZE;

// in the implementations below we do not implement addressing byte 2^32 - 1

impl<A: Allocator, B: Allocator> HostHeap for Vec<Vec<u8, A>, B> {
    fn num_pages(&self) -> u32 {
        self.len() as u32
    }
    fn copy_into_memory(&mut self, src: &[u8], offset: u32) -> Result<(), ()> {
        let mem_len = PAGE_SIZE * self.len();
        let src_len = u32::try_from(src.len()).map_err(|_| ())?;
        let end = src_len.checked_add(offset).ok_or(())?;
        if end > mem_len as u32 {
            return Err(());
        }

        if src_len == 0 {
            return Ok(());
        }

        let mut src_offset = 0_usize;
        let mut dst_offset = offset as usize;
        let mut len = src_len as usize;

        // we will proceed by identifying per-page subranges that can be copied with just "copy_from_slice"
        loop {
            let dst_page = dst_offset / PAGE_SIZE;
            let dst_in_page_offset = dst_offset % PAGE_SIZE;
            let dst_in_page_len = PAGE_SIZE - dst_offset % PAGE_SIZE;

            let len_to_copy = core::cmp::min(dst_in_page_len, len);

            // we have to do ptr::copy due to borrow checker
            unsafe {
                core::ptr::copy(
                    src[src_offset..].as_ptr(),
                    self[dst_page][dst_in_page_offset..].as_mut_ptr(),
                    len_to_copy,
                );
            }

            src_offset += len_to_copy;
            dst_offset += len_to_copy;
            len -= len_to_copy;
            if len == 0 {
                break;
            }
        }

        Ok(())
    }

    fn mem_read_into_slice(&self, dst: &mut [u8], offset: u32) -> Result<(), ()> {
        let num_bytes = dst.len() as u32;
        let mem_len = PAGE_SIZE * self.len();
        let end = offset.checked_add(num_bytes).ok_or(())?;
        if end > mem_len as u32 {
            return Err(());
        }

        if num_bytes == 0 {
            return Ok(());
        }

        let mut src_offset = offset as usize;
        let mut dst_offset = 0_usize;
        let mut len = num_bytes as usize;

        // we will proceed by identifying per-page subranges that can be copied with just "copy_from_slice"
        loop {
            let src_in_page_len = PAGE_SIZE - src_offset % PAGE_SIZE;
            let len_to_copy = core::cmp::min(src_in_page_len, len);

            let src_page = src_offset / PAGE_SIZE;
            let src_in_page_offset = src_offset % PAGE_SIZE;

            // we have to do ptr::copy due to borrow checker
            unsafe {
                core::ptr::copy(
                    self[src_page][src_in_page_offset..].as_ptr(),
                    dst[dst_offset..].as_mut_ptr(),
                    len_to_copy,
                );
            }

            src_offset += len_to_copy;
            dst_offset += len_to_copy;
            len -= len_to_copy;
            if len == 0 {
                break;
            }
        }

        Ok(())
    }

    fn fill_memory(&mut self, byte: u8, offset: u32, len: u32) -> Result<(), ()> {
        let mem_len = PAGE_SIZE * self.len();
        let end = offset.checked_add(len).ok_or(())?;
        if end > mem_len as u32 {
            return Err(());
        }

        if len == 0 {
            return Ok(());
        }

        let mut dst_offset = offset as usize;
        let mut len = len as usize;

        // we will proceed by identifying per-page subranges that can be copied with just "copy_from_slice"
        loop {
            let dst_in_page_len = PAGE_SIZE - dst_offset % PAGE_SIZE;
            let len_to_copy = core::cmp::min(dst_in_page_len, len);

            let dst_page = dst_offset / PAGE_SIZE;
            let dst_in_page_offset = dst_offset % PAGE_SIZE;

            self[dst_page][dst_in_page_offset..][..len_to_copy].fill(byte);

            dst_offset += len_to_copy;
            len -= len_to_copy;
            if len == 0 {
                break;
            }
        }

        Ok(())
    }

    fn copy_memory(&mut self, src_offset: u32, dst_offset: u32, len: u32) -> Result<(), ()> {
        let mem_len = PAGE_SIZE * self.len();
        let src_end = src_offset.checked_add(len).ok_or(())?;
        if src_end > mem_len as u32 {
            return Err(());
        }

        let dst_end = dst_offset.checked_add(len).ok_or(())?;
        if dst_end > mem_len as u32 {
            return Err(());
        }

        if len == 0 {
            return Ok(());
        }

        let mut src_offset = src_offset as usize;
        let mut dst_offset = dst_offset as usize;
        let mut len = len as usize;

        // we will proceed by identifying per-page subranges that can be copied with just "copy_from_slice"
        loop {
            let src_in_page_len = PAGE_SIZE - src_offset % PAGE_SIZE;
            let dst_in_page_len = PAGE_SIZE - dst_offset % PAGE_SIZE;

            let len_to_copy = core::cmp::min(src_in_page_len, dst_in_page_len);
            let len_to_copy = core::cmp::min(len_to_copy, len);

            let src_page = src_offset / PAGE_SIZE;
            let src_in_page_offset = src_offset % PAGE_SIZE;
            let dst_page = dst_offset / PAGE_SIZE;
            let dst_in_page_offset = dst_offset % PAGE_SIZE;

            // we have to do ptr::copy due to borrow checker
            unsafe {
                core::ptr::copy(
                    self[src_page][src_in_page_offset..].as_ptr(),
                    self[dst_page][dst_in_page_offset..].as_mut_ptr(),
                    len_to_copy,
                );
            }

            src_offset += len_to_copy;
            dst_offset += len_to_copy;
            len -= len_to_copy;
            if len == 0 {
                break;
            }
        }

        Ok(())
    }
}

impl<'this, T: Sized + 'this> ScratchSpaceRef<'this, T> for &'this [T] {}
impl<'this, T: Sized + 'this> ScratchSpaceCopiableRef<'this, T> for &'this [T] {}
impl<'this, T: Sized + 'this> ScratchSpaceRef<'this, T> for &'this mut [T] {}
impl<'this, T: Sized + 'this> ScratchSpaceRefMut<'this, T> for &'this mut [T] {}

// impl<'this, T: Sized + 'this> ScratchSpaceRef<'this, T> for &'this mut [T] {
//     fn is_empty(&self) -> bool {
//         <[T]>::is_empty(self)
//     }
//     fn len(&self) -> usize {
//         <[T]>::len(self)
//     }
//     fn get(&self, index: usize) -> Option<&T> {
//         self[..].get(index)
//     }
//     unsafe fn get_unchecked(&self, index: usize) -> &T {
//         self[..].get_unchecked(index)
//     }
//     unsafe fn last_unchecked(&self) -> &T {
//         self[..].get_unchecked(self.len() - 1)
//     }
//     unsafe fn get_slice_unchecked(&self, range: core::ops::Range<usize>) -> &[T] {
//         &self[range]
//     }
//     fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T>
//     where
//         Self: 'a,
//         T: 'a,
//     {
//         self[..].iter()
//     }
// }

// impl<'a, T: Sized + 'a> ScratchSpaceRefMut<'a, T> for &'a mut [T] {
//     unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
//         self[..].get_unchecked_mut(index)
//     }
//     unsafe fn last_unchecked_mut(&mut self) -> &mut T {
//         let len = self.len();
//         self[..].get_unchecked_mut(len - 1)
//     }
//     unsafe fn get_slice_unchecked_mut(&mut self, range: core::ops::Range<usize>) -> &mut [T] {
//         &mut self[range]
//     }
// }

impl<T: Sized, A: Allocator> ContinuousIndexAccess<T> for Vec<T, A> {
    fn is_empty(&self) -> bool {
        Vec::is_empty(self)
    }
    fn len(&self) -> usize {
        Vec::len(self)
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

impl<T: Sized, A: Allocator> ContinuousIndexAccessMut<T> for Vec<T, A> {
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

impl<T: Sized, A: Allocator> ScratchSpace<T> for Vec<T, A> {
    type Ref<'a>
        = &'a [T]
    where
        T: 'a,
        Self: 'a;
    type RefMut<'a>
        = &'a mut [T]
    where
        T: 'a,
        Self: 'a;

    fn by_ref(&self) -> Self::Ref<'_> {
        &self[..]
    }

    fn by_mut_ref(&mut self) -> Self::RefMut<'_> {
        &mut self[..]
    }

    fn clear(&mut self) {
        Vec::clear(self);
    }
    fn truncate(&mut self, len: usize) {
        Vec::truncate(self, len)
    }
    fn push(&mut self, el: T) -> Result<(), ()> {
        Vec::push_within_capacity(self, el).map_err(|_| ())
    }
    fn pop(&mut self) -> Option<T> {
        Vec::pop(self)
    }
    fn put_many(&mut self, el: T, num_elements: usize) -> Result<(), ()>
    where
        T: Clone,
    {
        let new_len = self.len() + num_elements;
        self.resize(new_len, el);
        Ok(())
    }
    fn drain_all(&mut self) -> impl Iterator<Item = T> {
        Vec::drain(self, ..)
    }
}

impl<T: Sized, A: Allocator> OutputBuffer<T> for Vec<T, A> {
    type Ref<'a>
        = &'a [T]
    where
        T: 'a,
        Self: 'a;
    fn by_ref(&self) -> Self::Ref<'_> {
        &self[..]
    }

    fn push(&mut self, el: T) -> Result<(), ()> {
        Vec::push_within_capacity(self, el).map_err(|_| ())
    }
}

impl SystemMemoryManager for () {
    type Allocator = Global;

    type ScratchSpace<T: Sized> = Vec<T, Global>;
    type OutputBuffer<T: Sized> = Vec<T, Global>;

    fn empty_scratch_space<T: Sized>(&self) -> Self::ScratchSpace<T> {
        Vec::new()
    }
    fn get_allocator(&self) -> Self::Allocator {
        Global
    }
    fn allocate_output_buffer<T: Sized>(
        &mut self,
        capacity: usize,
    ) -> Result<Self::OutputBuffer<T>, ()> {
        Ok(Vec::with_capacity(capacity))
    }

    fn allocate_scratch_space<T: Sized>(
        &mut self,
        capacity: usize,
    ) -> Result<Self::ScratchSpace<T>, ()> {
        Ok(Vec::with_capacity(capacity))
    }
    fn clone_scratch_space<T: Clone>(
        &mut self,
        existing_scratch: &Self::ScratchSpace<T>,
    ) -> Result<Self::ScratchSpace<T>, ()> {
        Ok(existing_scratch.clone())
    }
}

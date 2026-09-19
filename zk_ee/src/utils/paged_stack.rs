//! A stack stored in fixed-size pages.
//!
//! Pushing never moves elements and never reallocates: a full page links to the next one, and
//! pages stay allocated across [`PagedStack::clear`] for reuse, so after the first peak no
//! allocation happens at all. This suits the proving-mode allocator, which is allocate-only
//! and forbids growing an allocation.

use alloc::alloc::{handle_alloc_error, Layout};
use core::alloc::Allocator;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ptr::NonNull;

struct Page<T, const N: usize> {
    slots: [MaybeUninit<T>; N],
    prev: Option<NonNull<Page<T, N>>>,
    next: Option<NonNull<Page<T, N>>>,
}

pub struct PagedStack<T, const N: usize, A: Allocator + Clone> {
    /// The first page ever allocated; all pages are reachable from it through `next`
    first: Option<NonNull<Page<T, N>>>,
    /// The page holding the top of the stack; `None` when the stack is empty. Every page
    /// before it is full, and it holds `current_len` (at least one) elements.
    current: Option<NonNull<Page<T, N>>>,
    current_len: usize,
    len: usize,
    alloc: A,
}

// SAFETY: the pages are owned by the stack and only reached through it, so it can be sent or
// shared exactly when its elements and allocator can.
unsafe impl<T: Send, const N: usize, A: Allocator + Clone + Send> Send for PagedStack<T, N, A> {}
unsafe impl<T: Sync, const N: usize, A: Allocator + Clone + Sync> Sync for PagedStack<T, N, A> {}

impl<T, const N: usize, A: Allocator + Clone> PagedStack<T, N, A> {
    const PAGE_LAYOUT: Layout = Layout::new::<Page<T, N>>();

    pub fn empty(alloc: A) -> Self {
        const { assert!(N > 0, "PagedStack page capacity N must be non-zero") };
        Self {
            first: None,
            current: None,
            current_len: 0,
            len: 0,
            alloc,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Allocates a page with uninitialized slots, linked after `prev`
    fn alloc_page(&self, prev: Option<NonNull<Page<T, N>>>) -> NonNull<Page<T, N>> {
        let page = match self.alloc.allocate(Self::PAGE_LAYOUT) {
            Ok(ptr) => ptr.cast::<Page<T, N>>(),
            Err(_) => handle_alloc_error(Self::PAGE_LAYOUT),
        };
        // SAFETY: a fresh allocation of the page layout; only the links are initialized, the
        // slots are `MaybeUninit`
        unsafe {
            core::ptr::addr_of_mut!((*page.as_ptr()).prev).write(prev);
            core::ptr::addr_of_mut!((*page.as_ptr()).next).write(None);
        }
        page
    }

    /// Slot `index` of `page`
    fn slot(page: NonNull<Page<T, N>>, index: usize) -> *mut MaybeUninit<T> {
        debug_assert!(index < N);
        // SAFETY: `page` is a live page and `index` is within its slots
        unsafe {
            core::ptr::addr_of_mut!((*page.as_ptr()).slots)
                .cast::<MaybeUninit<T>>()
                .add(index)
        }
    }

    pub fn push(&mut self, value: T) {
        let page = match self.current {
            None => {
                let page = match self.first {
                    Some(page) => page,
                    None => {
                        let page = self.alloc_page(None);
                        self.first = Some(page);
                        page
                    }
                };
                self.current_len = 0;
                page
            }
            Some(current) if self.current_len == N => {
                // SAFETY: `current` is a live page
                let next = match unsafe { (*current.as_ptr()).next } {
                    Some(next) => next,
                    None => {
                        let next = self.alloc_page(Some(current));
                        // SAFETY: `current` is a live page
                        unsafe { (*current.as_ptr()).next = Some(next) };
                        next
                    }
                };
                self.current_len = 0;
                next
            }
            Some(current) => current,
        };
        self.current = Some(page);
        // SAFETY: a free slot of the page (`current_len < N`)
        unsafe { Self::slot(page, self.current_len).write(MaybeUninit::new(value)) };
        self.current_len += 1;
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        let page = self.current?;
        debug_assert!(self.current_len >= 1);
        self.current_len -= 1;
        self.len -= 1;
        // SAFETY: the slot holds an initialized element, which is moved out
        let value = unsafe { Self::slot(page, self.current_len).read().assume_init() };
        if self.current_len == 0 {
            // SAFETY: `page` is a live page
            self.current = unsafe { (*page.as_ptr()).prev };
            self.current_len = if self.current.is_some() { N } else { 0 };
        }
        Some(value)
    }

    /// The elements, newest first
    pub fn iter(&self) -> PagedStackIter<'_, T, N> {
        PagedStackIter {
            page: self.current,
            index: self.current_len,
            remaining: self.len,
            _borrow: PhantomData,
        }
    }

    /// Drops the elements; the pages stay allocated for reuse
    pub fn clear(&mut self) {
        if core::mem::needs_drop::<T>() {
            while self.pop().is_some() {}
        }
        self.current = None;
        self.current_len = 0;
        self.len = 0;
    }
}

impl<T, const N: usize, A: Allocator + Clone> Drop for PagedStack<T, N, A> {
    fn drop(&mut self) {
        self.clear();
        let mut page = self.first;
        while let Some(current) = page {
            // SAFETY: a live page allocated with `PAGE_LAYOUT`, freed exactly once; its
            // elements were dropped by `clear`
            unsafe {
                page = (*current.as_ptr()).next;
                self.alloc
                    .deallocate(current.cast::<u8>(), Self::PAGE_LAYOUT);
            }
        }
    }
}

pub struct PagedStackIter<'a, T, const N: usize> {
    page: Option<NonNull<Page<T, N>>>,
    /// Elements not yet yielded in `page`
    index: usize,
    remaining: usize,
    _borrow: PhantomData<&'a T>,
}

impl<'a, T, const N: usize> Iterator for PagedStackIter<'a, T, N> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        if self.remaining == 0 {
            return None;
        }
        let mut page = self.page?;
        if self.index == 0 {
            // SAFETY: a live page; there are remaining elements, so it has a predecessor
            page = unsafe { (*page.as_ptr()).prev }?;
            self.page = Some(page);
            self.index = N;
        }
        self.index -= 1;
        self.remaining -= 1;
        // SAFETY: an initialized slot, borrowed for the life of the stack borrow
        Some(unsafe {
            &*PagedStack::<T, N, alloc::alloc::Global>::slot(page, self.index).cast::<T>()
        })
    }
}

impl<'a, T, const N: usize, A: Allocator + Clone> IntoIterator for &'a PagedStack<T, N, A> {
    type Item = &'a T;
    type IntoIter = PagedStackIter<'a, T, N>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::PagedStack;
    use alloc::alloc::Global;
    use alloc::vec::Vec;

    #[test]
    fn push_pop_across_pages() {
        let mut stack = PagedStack::<u32, 4, Global>::empty(Global);
        assert!(stack.is_empty());
        for i in 0..10 {
            stack.push(i);
        }
        assert_eq!(stack.len(), 10);
        let newest_first: Vec<u32> = stack.iter().copied().collect();
        assert_eq!(newest_first, (0..10).rev().collect::<Vec<_>>());
        for i in (0..10).rev() {
            assert_eq!(stack.pop(), Some(i));
        }
        assert_eq!(stack.pop(), None);
        assert!(stack.is_empty());
        assert_eq!(stack.iter().count(), 0);
    }

    #[test]
    fn interleaved_and_cleared() {
        let mut stack = PagedStack::<u32, 3, Global>::empty(Global);
        for round in 0..5 {
            for i in 0..7 {
                stack.push(round * 100 + i);
            }
            assert_eq!(stack.pop(), Some(round * 100 + 6));
            stack.push(round * 100 + 60);
            assert_eq!(stack.pop(), Some(round * 100 + 60));
            assert_eq!(stack.pop(), Some(round * 100 + 5));
            assert_eq!(stack.pop(), Some(round * 100 + 4));
            assert_eq!(stack.len(), 4);
            let rest: Vec<u32> = stack.iter().copied().collect();
            assert_eq!(rest, [3, 2, 1, 0].map(|i| round * 100 + i));
            stack.clear();
            assert!(stack.is_empty());
        }
    }

    #[test]
    fn drops_elements() {
        use core::cell::Cell;
        struct Counted<'a>(&'a Cell<usize>);
        impl Drop for Counted<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let drops = Cell::new(0);
        {
            let mut stack = PagedStack::<Counted, 2, Global>::empty(Global);
            for _ in 0..5 {
                stack.push(Counted(&drops));
            }
            drop(stack.pop());
            assert_eq!(drops.get(), 1);
            stack.clear();
            assert_eq!(drops.get(), 5);
            stack.push(Counted(&drops));
        }
        assert_eq!(drops.get(), 6);
    }
}

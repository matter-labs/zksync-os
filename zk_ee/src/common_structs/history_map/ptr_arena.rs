//! A paged bump arena that hands out provenance-stable pointers.
//!
//! # Why this exists
//!
//! The history map stores `NonNull` pointers into its backing storage and keeps
//! using them (read *and* write) across later insertions. That requires two
//! properties from the storage:
//!
//! 1. **Stable addresses** — a value never moves once allocated.
//! 2. **Stable provenance** — a handed-out pointer stays valid for writes across
//!    later arena operations.
//!
//! A `ListVec` (`LinkedList<ArrayVec<T, N>>`) gives (1) but not (2): every
//! `push`/`top_mut`/`back_mut` reborrows a node mutably (`iter_mut().last()` /
//! `back_mut()`), and under both Stacked and Tree Borrows that reborrow acts as
//! a foreign access that invalidates pointers handed out earlier — making any
//! later write through them Undefined Behavior.
//!
//! `PtrArena` fixes (2) by owning each page as a raw allocation and deriving
//! *every* slot pointer — both the append write and the returned handle — from
//! that one raw page pointer via offset arithmetic. No `&mut`/`&` reference is
//! ever formed over the page's payload, so appending never reasserts exclusive
//! access over previously handed-out slots: the handles stay valid for the
//! lifetime of the arena.

use alloc::collections::LinkedList;
use core::alloc::{Allocator, Layout};
use core::mem::MaybeUninit;
use core::ptr::NonNull;

/// One fixed-capacity page: a raw allocation of `N` slots plus the count of
/// initialized slots at the front. The payload is reached only through `data`
/// (a raw pointer with provenance over the whole page); we never form a
/// reference to it.
struct Page<T, const N: usize> {
    data: NonNull<MaybeUninit<T>>,
    len: usize,
}

/// A bump arena storing `T` in a chain of `N`-slot pages.
///
/// `push` returns a `NonNull<T>` that remains valid (for reads and writes) until
/// [`PtrArena::clear`] or drop, regardless of subsequent `push` calls. Slots are
/// never freed or moved individually.
pub struct PtrArena<T, const N: usize, A: Allocator> {
    /// Page metadata, in a `LinkedList` (not a `Vec`): the proving-mode
    /// allocator is allocate-only and forbids `realloc`/`grow`, so a growable
    /// `Vec` would panic ("grow is not allowed") once it outgrew its capacity.
    /// A `LinkedList` only ever `allocate`s one node per page. Reborrowing a
    /// node (e.g. `back_mut`) is harmless: the page *payload* is a separate
    /// allocation reached via `Page::data`, so handed-out element pointers are
    /// never invalidated.
    pages: LinkedList<Page<T, N>, A>,
    alloc: A,
}

impl<T, const N: usize, A: Allocator> PtrArena<T, N, A> {
    /// Layout of a single page payload (`[MaybeUninit<T>; N]`). Infallible:
    /// `N` is a fixed positive constant and the element type is sized.
    const PAGE_LAYOUT: Layout = Layout::new::<[MaybeUninit<T>; N]>();

    pub fn new_in(alloc: A) -> Self
    where
        A: Clone,
    {
        Self {
            pages: LinkedList::new_in(alloc.clone()),
            alloc,
        }
    }

    /// Allocates a fresh page payload and returns a raw pointer with provenance
    /// over the entire page. Panics on allocation failure, matching the
    /// allocate-or-abort behavior of the `Vec`/`ListVec` storage it replaces.
    fn alloc_page(&self) -> NonNull<MaybeUninit<T>> {
        match self.alloc.allocate(Self::PAGE_LAYOUT) {
            Ok(ptr) => ptr.cast::<MaybeUninit<T>>(),
            Err(_) => alloc::alloc::handle_alloc_error(Self::PAGE_LAYOUT),
        }
    }

    /// Moves `value` into the arena and returns a stable pointer to it.
    pub fn push(&mut self, value: T) -> NonNull<T> {
        let need_new_page = self.pages.back().is_none_or(|page| page.len == N);
        if need_new_page {
            let data = self.alloc_page();
            self.pages.push_back(Page { data, len: 0 });
        }

        // The borrow of `page` is over the metadata struct only; the payload is
        // touched solely through the raw `page.data` pointer below.
        let page = self
            .pages
            .back_mut()
            .expect("a page exists: just pushed one if needed");

        // SAFETY: `page.len < N` (a new page was pushed above when the last one
        // was full), so the slot lies within the page allocation. The pointer
        // is derived from `page.data`, carrying provenance over the whole page;
        // `write` initializes the slot without forming a reference to it.
        let slot = unsafe { page.data.as_ptr().add(page.len).cast::<T>() };
        unsafe { slot.write(value) };
        page.len += 1;

        // SAFETY: `slot` is non-null (derived from the non-null page pointer)
        // and now points at an initialized `T`.
        unsafe { NonNull::new_unchecked(slot) }
    }

    /// Drops every initialized element and releases all page allocations,
    /// leaving the arena empty and reusable.
    pub fn clear(&mut self) {
        // Drains the metadata list; each page's elements are dropped and its
        // payload deallocated.
        while let Some(page) = self.pages.pop_front() {
            for i in 0..page.len {
                // SAFETY: slots `0..len` are initialized; each physical slot is
                // dropped exactly once (the arena is the sole owner of the
                // payload — the history chains/free list only hold borrows-as
                // -pointers into it).
                unsafe { page.data.as_ptr().add(i).cast::<T>().drop_in_place() };
            }
            // SAFETY: `page.data` came from `alloc_page` with `PAGE_LAYOUT` and
            // is freed exactly once here.
            unsafe {
                self.alloc
                    .deallocate(page.data.cast::<u8>(), Self::PAGE_LAYOUT)
            };
        }
    }
}

impl<T, const N: usize, A: Allocator> Drop for PtrArena<T, N, A> {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::alloc::AllocError;
    use std::alloc::Global;
    use std::cell::Cell;

    /// Allocator that forwards to `Global` but panics on any realloc, mirroring
    /// the proving-mode allocator's allocate-only contract. `PtrArena` must
    /// only ever `allocate` (new pages + new metadata nodes), never `grow`.
    #[derive(Clone)]
    struct NoGrowAlloc;
    // SAFETY: forwards to the global allocator; blocks only resize the way the
    // proving allocator does.
    unsafe impl Allocator for NoGrowAlloc {
        fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
            Global.allocate(layout)
        }
        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            unsafe { Global.deallocate(ptr, layout) }
        }
        unsafe fn grow(
            &self,
            _ptr: NonNull<u8>,
            _old: Layout,
            _new: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            panic!("grow is not allowed (mirrors proving-mode allocator)");
        }
        unsafe fn grow_zeroed(
            &self,
            _ptr: NonNull<u8>,
            _old: Layout,
            _new: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            panic!("grow_zeroed is not allowed (mirrors proving-mode allocator)");
        }
        unsafe fn shrink(
            &self,
            _ptr: NonNull<u8>,
            _old: Layout,
            _new: Layout,
        ) -> Result<NonNull<[u8]>, AllocError> {
            panic!("shrink is not allowed (mirrors proving-mode allocator)");
        }
    }

    #[test]
    fn push_never_reallocs_across_many_pages() {
        // Regression guard: the proving allocator forbids realloc. Spanning many
        // pages (here 50 elems over capacity-4 pages = 13 pages) must allocate
        // page payloads and metadata nodes only — never grow. A `Vec`-backed
        // page list would panic here once it outgrew its capacity.
        let mut arena = PtrArena::<usize, 4, NoGrowAlloc>::new_in(NoGrowAlloc);
        let mut ptrs = std::vec::Vec::new();
        for k in 0..50usize {
            ptrs.push(arena.push(k));
        }
        for (k, mut p) in ptrs.iter().copied().enumerate() {
            assert_eq!(unsafe { *p.as_ref() }, k);
            unsafe { *p.as_mut() += 1 };
        }
        for (k, p) in ptrs.iter().copied().enumerate() {
            assert_eq!(unsafe { *p.as_ref() }, k + 1);
        }
    }

    #[test]
    fn push_returns_stable_writable_pointers_across_pages() {
        // Page capacity 4: 10 elements spans 3 pages with a partial last page.
        let mut arena = PtrArena::<usize, 4, Global>::new_in(Global);

        let mut ptrs = std::vec::Vec::new();
        for k in 0..10usize {
            ptrs.push(arena.push(k));
        }

        // Every earlier pointer (including those on now-"sealed" pages) is still
        // valid for reads and writes after all the appends that followed it.
        for (k, mut p) in ptrs.iter().copied().enumerate() {
            assert_eq!(unsafe { *p.as_ref() }, k);
            unsafe { *p.as_mut() += 100 };
        }
        for (k, p) in ptrs.iter().copied().enumerate() {
            assert_eq!(unsafe { *p.as_ref() }, k + 100);
        }
    }

    #[test]
    fn clear_runs_destructors_exactly_once_then_reuses() {
        // A type that bumps a shared counter on drop, to assert exactly-once.
        struct DropCounter<'a>(&'a Cell<usize>);
        impl Drop for DropCounter<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let drops = Cell::new(0);
        let mut arena = PtrArena::<DropCounter, 4, Global>::new_in(Global);
        for _ in 0..10 {
            arena.push(DropCounter(&drops));
        }

        arena.clear();
        assert_eq!(drops.get(), 10, "every initialized slot dropped once");

        // Arena is reusable after clear.
        let p = arena.push(DropCounter(&drops));
        assert_eq!(drops.get(), 10);
        drop(p);
        drop(arena);
        assert_eq!(
            drops.get(),
            11,
            "the post-clear element dropped on arena drop"
        );
    }
}

//! Arena allocator for `ElementWithHistory` values.
//!
//! Wraps a `ListVec<ElementWithHistory<...>, N, A>` so the underlying values
//! get a stable in-memory address (the `ArrayVec` slots never relocate, and
//! new pages are appended to the linked list without touching existing
//! pages). Storing `NonNull` pointers from this arena in the `HistoryMap`'s
//! BTreeMap and pending-updates list bypasses BTreeMap descents on the
//! rollback/commit/iter-pending paths and avoids per-element heap allocations.

use core::{alloc::Allocator, ptr::NonNull};

use crate::{common_structs::skip_list_quasi_vec::ListVec, memory::stack_trait::Stack};

use super::element_with_history::ElementWithHistory;

/// Number of `ElementWithHistory` slots per backing page. Sized so a page
/// fits within a small handful of cache lines for the K/V types in use
/// (~24-52 B keys, ~32 B head/initial/first/committed pointers, plus
/// optional element properties).
const ELEMENT_PAGE_CAPACITY: usize = 32;

pub struct ElementWithHistoryArena<K, V, A: Allocator + Clone, KP> {
    buffer: ListVec<ElementWithHistory<K, V, A, KP>, ELEMENT_PAGE_CAPACITY, A>,
}

impl<K, V, A: Allocator + Clone, KP> ElementWithHistoryArena<K, V, A, KP> {
    pub fn new(alloc: A) -> Self {
        Self {
            buffer: ListVec::new_in(alloc),
        }
    }

    /// Moves `element` into the arena and returns a stable pointer to it.
    pub fn allocate(
        &mut self,
        element: ElementWithHistory<K, V, A, KP>,
    ) -> NonNull<ElementWithHistory<K, V, A, KP>> {
        self.buffer.push(element);
        let slot = self
            .buffer
            .top_mut()
            .expect("ListVec::top_mut must yield the slot we just pushed");
        NonNull::from(&mut *slot)
    }

    /// Drops every element and releases the arena pages.
    pub fn clear(&mut self) {
        // LinkedList::clear drops the ArrayVec nodes, which in turn drop the
        // contained ElementWithHistory values (and their owned keys).
        self.buffer.0.clear();
    }
}

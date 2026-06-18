//! Arena allocator for `ElementWithHistory` values.
//!
//! Wraps a [`PtrArena`] so the underlying values get a stable in-memory address
//! *and* stable provenance: the `NonNull` pointers stored in the `HistoryMap`'s
//! BTreeMap and pending-updates list stay valid for reads and writes across
//! later allocations. This bypasses BTreeMap descents on the
//! rollback/commit/iter-pending paths and amortizes per-element allocation over
//! arena pages.

use core::{alloc::Allocator, ptr::NonNull};

use super::element_with_history::ElementWithHistory;
use super::ptr_arena::PtrArena;

/// Number of `ElementWithHistory` slots per backing page. Sized so a page
/// fits within a small handful of cache lines for the K/V types in use
/// (~24-52 B keys, ~32 B head/initial/first/committed pointers, plus
/// optional element properties).
pub(crate) const ELEMENT_PAGE_CAPACITY: usize = 32;

pub struct ElementWithHistoryArena<K, V, A: Allocator + Clone, KP> {
    buffer: PtrArena<ElementWithHistory<K, V, A, KP>, ELEMENT_PAGE_CAPACITY, A>,
}

impl<K, V, A: Allocator + Clone, KP> ElementWithHistoryArena<K, V, A, KP> {
    pub fn new(alloc: A) -> Self {
        Self {
            buffer: PtrArena::new_in(alloc),
        }
    }

    /// Moves `element` into the arena and returns a stable pointer to it.
    pub fn allocate(
        &mut self,
        element: ElementWithHistory<K, V, A, KP>,
    ) -> NonNull<ElementWithHistory<K, V, A, KP>> {
        self.buffer.push(element)
    }

    /// Drops every element (and its owned key) and releases the arena pages.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

use super::{
    element_with_history::{HistoryRecord, HistoryRecordLink},
    ptr_arena::PtrArena,
    CacheSnapshotId,
};
use core::alloc::Allocator;

/// Manages memory allocations for history records, reuses old allocations for optimization
pub struct ElementPool<V, A: Allocator + Clone> {
    /// Head of `recycled` sub-list
    head: Option<HistoryRecordLink<V>>,
    /// Tail of `recycled` sub-list
    last: Option<HistoryRecordLink<V>>,
    /// Stable-address, stable-provenance storage for the records. Handed-out
    /// `HistoryRecordLink`s stay valid for writes across later allocations.
    buffer: PtrArena<HistoryRecord<V>, 50, A>,
}

impl<V, A: Allocator + Clone> ElementPool<V, A> {
    pub fn new(alloc: A) -> Self {
        Self {
            head: Default::default(),
            last: Default::default(),
            buffer: PtrArena::new_in(alloc),
        }
    }

    /// Allocate memory or reuse old record and create a new record
    pub fn create_element(
        &mut self,
        value: V,
        previous: Option<HistoryRecordLink<V>>,
        snapshot_id: CacheSnapshotId,
    ) -> HistoryRecordLink<V> {
        match self.head {
            None => {
                // Bump-allocate a fresh record. The returned pointer carries
                // page-wide provenance (see `PtrArena`) so later `as_mut()`
                // writes through it — here, in `reuse_memory`, `rollback`,
                // `commit` — are sound.
                self.buffer.push(HistoryRecord {
                    touch_ss_id: snapshot_id,
                    value,
                    previous,
                })
            }
            Some(mut elem) => {
                // Reuse old allocation
                {
                    let elem = unsafe { elem.as_mut() };

                    self.head = elem.previous.take();

                    if self.head.is_none() {
                        self.last = None;
                    }

                    // Safety: We *must* rewrite all the links in `elem`.
                    elem.touch_ss_id = snapshot_id;
                    elem.value = value;
                    elem.previous = previous;
                }

                elem
            }
        }
    }

    /// Store a chain of records to reuse them later
    pub fn reuse_memory(
        &mut self,
        chain_head: HistoryRecordLink<V>,
        mut chain_tail: HistoryRecordLink<V>,
    ) {
        match self.last {
            None => {
                self.head = Some(chain_head);
            }
            Some(ref mut last) => {
                unsafe { last.as_mut().previous = Some(chain_head) };
            }
        }

        // We need to unlink this, cause it still points to the original history it's been taken
        // from.
        unsafe { chain_tail.as_mut().previous = None };

        self.last = Some(chain_tail);
    }
}

#[cfg(test)]
mod tests {
    use crate::common_structs::history_map::CacheSnapshotId;
    use std::alloc::Global;

    use super::ElementPool;

    #[test]
    fn creates_new_element() {
        let mut elements_pool: ElementPool<u32, Global> = ElementPool::new(Global);
        let element = elements_pool.create_element(11, None, CacheSnapshotId(1));

        assert_eq!(unsafe { element.as_ref().value }, 11);
        assert_eq!(unsafe { element.as_ref().touch_ss_id }, CacheSnapshotId(1));
    }

    #[test]
    fn creates_new_element_reusing_memory() {
        let mut elements_pool: ElementPool<u32, Global> = ElementPool::new(Global);
        let element = elements_pool.create_element(11, None, CacheSnapshotId(1));

        elements_pool.reuse_memory(element, element);

        assert!(elements_pool.head != None);

        let element = elements_pool.create_element(2, None, CacheSnapshotId(10));
        assert_eq!(unsafe { element.as_ref().value }, 2);
        assert_eq!(unsafe { element.as_ref().touch_ss_id }, CacheSnapshotId(10));
    }
}

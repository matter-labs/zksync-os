use super::{element_pool::ElementPool, CacheSnapshotId};
use alloc::boxed::Box;
use core::{alloc::Allocator, ptr::NonNull};

pub type HistoryRecordLink<V> = NonNull<HistoryRecord<V>>;

/// Record in some element's history
pub struct HistoryRecord<V> {
    pub touch_ss_id: CacheSnapshotId,
    pub value: V,
    pub previous: Option<HistoryRecordLink<V>>,
}

/// The history linked list. Always has at least one item with the snapshot id of 0.
pub struct ElementWithHistory<V, A: Allocator + Clone> {
    /// Initial record (before history started)
    pub initial: HistoryRecordLink<V>,
    pub first: HistoryRecordLink<V>,
    /// Current history record
    pub head: HistoryRecordLink<V>,
    alloc: A,
}

impl<V, A: Allocator + Clone> Drop for ElementWithHistory<V, A> {
    fn drop(&mut self) {
        let mut elem = unsafe { Box::from_raw_in(self.head.as_ptr(), self.alloc.clone()) };

        while let Some(n) = elem.previous.take() {
            let n = unsafe { Box::from_raw_in(n.as_ptr(), self.alloc.clone()) };

            elem = n;
        } // `n` is dropped here.
    } // last elem is dropped here.
}

impl<V, A: Allocator + Clone> ElementWithHistory<V, A> {
    #[inline(always)]
    pub fn new(value: V, records_memory_pool: &mut ElementPool<V, A>, alloc: A) -> Self {
        // Note: initial value always has snapshot id 0
        let elem = records_memory_pool.create_element(value, None, CacheSnapshotId(0));

        Self {
            head: elem,
            initial: elem,
            first: elem,
            alloc,
        }
    }

    pub fn add_new_record(&mut self, new_element: HistoryRecordLink<V>) {
        self.head = new_element;
        if self.initial == self.first {
            // When don't have any updates before
            self.first = new_element;
        }
    }

    /// Rollback element's state to snapshot_id
    /// Removed history records stored in records_memory_pool to reuse later
    pub fn rollback(
        &mut self,
        records_memory_pool: &mut ElementPool<V, A>,
        snapshot_id: CacheSnapshotId,
    ) {
        // Caller should guarantee that snapshot_id is correct

        if unsafe { self.head.as_ref() }.touch_ss_id <= snapshot_id {
            return;
        }

        let mut first_removed_record = self.head;
        // Find first elem such that elem.touch_ss_id > snapshot_id and set previous as first_removed_record
        loop {
            let n_lnk = unsafe {
                first_removed_record
                    .as_mut()
                    .previous
                    .as_mut()
                    .expect("Every history is terminated with a 0'th snapshot")
            };

            let n = unsafe { n_lnk.as_mut() };

            if n.touch_ss_id <= snapshot_id {
                // This is guaranteed to happen by encountering the terminator snapshot.
                break;
            }

            first_removed_record = *n_lnk;
        }

        let last_removed_record = self.head;

        let new_head = unsafe { first_removed_record.as_mut() }
            .previous
            .take()
            .unwrap();

        if first_removed_record == self.first {
            self.first = new_head;
        }

        self.head = new_head;

        // Return subchain to the pool to be reused later
        records_memory_pool.reuse_memory(last_removed_record, first_removed_record);
    }

    /// Returns (initial_value, current_value) if any
    pub fn get_initial_and_last_values(&self) -> Option<(&V, &V)> {
        let entry = unsafe { self.head.as_ref() };
        match entry.previous {
            None => None,
            Some(_) => Some((unsafe { &self.initial.as_ref().value }, &entry.value)),
        }
    }

    /// Commits (freezes) changes up to this point
    /// Frees memory taken by snapshots that can't be rollbacked to.
    pub fn commit(&mut self, records_memory_pool: &mut ElementPool<V, A>) {
        // Case with only initial value (no writes at all)
        if self.head == self.initial {
            return;
        }

        // Current snapshot is the one we're committing to (only one update).
        if self.head == self.first {
            return;
        }

        // Safety: initial and first elements are distinct. Cases with 0-1 updates are covered above.

        let first_removed_record = self.first;

        // Previous head becomes new `first` record
        self.first = self.head;

        let head_mut = unsafe { self.head.as_mut() };
        let last_removed_record = head_mut
            .previous
            .replace(self.initial)
            .expect("History has at least 3 items.");

        // Return subchain to the pool to be reused later
        records_memory_pool.reuse_memory(last_removed_record, first_removed_record);
    }
}

#[cfg(test)]
mod tests {
    use crate::common_structs::history_map::CacheSnapshotId;
    use std::alloc::Global;

    use super::ElementPool;
    use super::ElementWithHistory;

    fn check_that_head_is_initial_element(
        expected_value: usize,
        element_with_history: &ElementWithHistory<usize, Global>,
    ) {
        assert_eq!(element_with_history.head, element_with_history.initial);
        assert_eq!(element_with_history.head, element_with_history.first);
        assert_eq!(
            unsafe { element_with_history.head.as_ref().value },
            expected_value
        );
        assert_eq!(unsafe { element_with_history.head.as_ref().previous }, None);
        assert_eq!(
            unsafe { element_with_history.head.as_ref().touch_ss_id },
            CacheSnapshotId(0)
        );
    }

    #[test]
    fn initializes_correctly() {
        let mut element_pool = ElementPool::new(Global);
        let element_with_history: ElementWithHistory<usize, Global> =
            ElementWithHistory::new(1, &mut element_pool, Global);

        check_that_head_is_initial_element(1, &element_with_history);
    }

    #[test]
    fn adds_new_records_and_rollbacks_them() {
        let mut element_pool = ElementPool::new(Global);
        let mut element_with_history: ElementWithHistory<usize, Global> =
            ElementWithHistory::new(1, &mut element_pool, Global);

        let first_element =
            element_pool.create_element(2, Some(element_with_history.head), CacheSnapshotId(1));
        element_with_history.add_new_record(first_element);

        assert_eq!(element_with_history.head, first_element);
        assert_eq!(element_with_history.first, first_element);

        let mut last_added_element = first_element;

        for n in 2..=100 {
            let new_element =
                element_pool.create_element(n + 1, Some(last_added_element), CacheSnapshotId(n));
            element_with_history.add_new_record(new_element);
            last_added_element = new_element;
        }

        element_with_history.rollback(&mut element_pool, CacheSnapshotId(2));

        assert_eq!(element_with_history.first, first_element);

        assert_eq!(unsafe { element_with_history.head.as_ref().value }, 3);
    }

    #[test]
    fn rollbacks_to_initial_as_head() {
        let mut element_pool = ElementPool::new(Global);
        let mut element_with_history: ElementWithHistory<usize, Global> =
            ElementWithHistory::new(1, &mut element_pool, Global);

        element_with_history.rollback(&mut element_pool, CacheSnapshotId(0));
        check_that_head_is_initial_element(1, &element_with_history);
    }

    #[test]
    fn rollbacks() {
        let mut element_pool = ElementPool::new(Global);
        let mut element_with_history: ElementWithHistory<usize, Global> =
            ElementWithHistory::new(1, &mut element_pool, Global);

        element_with_history.add_new_record(element_pool.create_element(
            2,
            Some(element_with_history.head),
            CacheSnapshotId(1),
        ));

        element_with_history.rollback(&mut element_pool, CacheSnapshotId(0));
        check_that_head_is_initial_element(1, &element_with_history);
    }

    #[test]
    fn commits_with_initial_value() {
        let mut element_pool = ElementPool::new(Global);
        let mut element_with_history: ElementWithHistory<usize, Global> =
            ElementWithHistory::new(1, &mut element_pool, Global);

        element_with_history.commit(&mut element_pool);
        check_that_head_is_initial_element(1, &element_with_history);
    }

    #[test]
    fn commits_one_record() {
        let mut element_pool = ElementPool::new(Global);
        let mut element_with_history: ElementWithHistory<usize, Global> =
            ElementWithHistory::new(1, &mut element_pool, Global);

        let new_element =
            element_pool.create_element(2, Some(element_with_history.head), CacheSnapshotId(1));

        element_with_history.add_new_record(new_element);

        element_with_history.commit(&mut element_pool);
        assert_eq!(element_with_history.head, new_element);
        assert_eq!(element_with_history.first, new_element);
    }

    #[test]
    fn commits_two_records() {
        let mut element_pool = ElementPool::new(Global);
        let mut element_with_history: ElementWithHistory<usize, Global> =
            ElementWithHistory::new(1, &mut element_pool, Global);

        let new_element =
            element_pool.create_element(2, Some(element_with_history.head), CacheSnapshotId(1));
        element_with_history.add_new_record(new_element);

        let new_element_2 = element_pool.create_element(3, Some(new_element), CacheSnapshotId(2));
        element_with_history.add_new_record(new_element_2);

        element_with_history.commit(&mut element_pool);

        assert_eq!(element_with_history.head, new_element_2);
        assert_eq!(element_with_history.first, new_element_2);
    }
}

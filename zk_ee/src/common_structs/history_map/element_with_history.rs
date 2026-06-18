use super::{record_pool::HistoryRecordPool, CacheSnapshotId};
use core::marker::PhantomData;
use core::{alloc::Allocator, ptr::NonNull};

pub type HistoryRecordLink<V> = NonNull<HistoryRecord<V>>;

/// Record in some element's history
pub struct HistoryRecord<V> {
    pub touch_ss_id: CacheSnapshotId,
    pub value: V,
    pub previous: Option<HistoryRecordLink<V>>,
}

/// The history linked list. Always has at least one item with the snapshot id of 0.
///
/// `key` is embedded so that the pending-updates list can store a stable pointer
/// to an `ElementWithHistory` and still surface the key on iteration without
/// going back through the BTreeMap.
pub struct ElementWithHistory<K, V, A: Allocator + Clone, EP = ()> {
    /// Key owned by this element (separate from the BTreeMap key copy).
    pub key: K,
    /// Additional properties associated with the element globally.
    /// These properties persist across rollbacks/commits and don't participate in snapshots
    pub element_properties: EP,
    /// Initial record (before history started)
    pub initial: HistoryRecordLink<V>,
    first: HistoryRecordLink<V>,
    /// Current history record
    pub head: HistoryRecordLink<V>,
    /// Record that has been committed, or initial if not commit has been
    /// performed.
    pub committed: HistoryRecordLink<V>,
    marker: PhantomData<A>,
}

impl<K, V, A: Allocator + Clone, KP> ElementWithHistory<K, V, A, KP> {
    #[inline(always)]
    pub fn new(
        key: K,
        key_properties: KP,
        initial_value: V,
        records_memory_pool: &mut HistoryRecordPool<V, A>,
    ) -> Self {
        // Note: initial value always has snapshot id 0
        let elem = records_memory_pool.create_record(initial_value, None, CacheSnapshotId(0));

        Self {
            key,
            element_properties: key_properties,
            head: elem,
            initial: elem,
            first: elem,
            committed: elem,
            marker: Default::default(),
        }
    }

    pub fn add_new_record(&mut self, new_record: HistoryRecordLink<V>) {
        self.head = new_record;
        if self.initial == self.first {
            // When don't have any updates before
            self.first = new_record;
        }
    }

    /// Rollback element's state to snapshot_id
    /// Removed history records stored in records_memory_pool to reuse later
    pub fn rollback(
        &mut self,
        records_memory_pool: &mut HistoryRecordPool<V, A>,
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
    /// Frees memory taken by snapshots that can't be rolled back to.
    pub fn commit(&mut self, records_memory_pool: &mut HistoryRecordPool<V, A>) {
        // Head becomes the committed value
        self.committed = self.head;

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

    use super::ElementWithHistory;
    use super::HistoryRecordPool;

    fn check_that_head_is_initial_record(
        expected_value: usize,
        element_with_history: &ElementWithHistory<(), usize, Global>,
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
        let mut record_pool = HistoryRecordPool::new(Global);
        let element_with_history: ElementWithHistory<(), usize, Global> =
            ElementWithHistory::new((), (), 1, &mut record_pool);

        check_that_head_is_initial_record(1, &element_with_history);

        assert_eq!(element_with_history.committed, element_with_history.initial);
    }

    #[test]
    fn adds_new_records_and_rollbacks_them() {
        let mut record_pool = HistoryRecordPool::new(Global);
        let mut element_with_history: ElementWithHistory<(), usize, Global> =
            ElementWithHistory::new((), (), 1, &mut record_pool);

        let first_record =
            record_pool.create_record(2, Some(element_with_history.head), CacheSnapshotId(1));
        element_with_history.add_new_record(first_record);

        assert_eq!(element_with_history.head, first_record);
        assert_eq!(element_with_history.first, first_record);

        let mut last_added_record = first_record;

        for n in 2..=100 {
            let new_record =
                record_pool.create_record(n + 1, Some(last_added_record), CacheSnapshotId(n));
            element_with_history.add_new_record(new_record);
            last_added_record = new_record;
        }

        element_with_history.rollback(&mut record_pool, CacheSnapshotId(2));

        assert_eq!(element_with_history.first, first_record);

        assert_eq!(unsafe { element_with_history.head.as_ref().value }, 3);

        assert_eq!(element_with_history.committed, element_with_history.initial);
    }

    #[test]
    fn rollbacks_to_initial_as_head() {
        let mut record_pool = HistoryRecordPool::new(Global);
        let mut element_with_history: ElementWithHistory<(), usize, Global> =
            ElementWithHistory::new((), (), 1, &mut record_pool);

        element_with_history.rollback(&mut record_pool, CacheSnapshotId(0));
        check_that_head_is_initial_record(1, &element_with_history);
        assert_eq!(element_with_history.committed, element_with_history.initial);
    }

    #[test]
    fn rollbacks() {
        let mut record_pool = HistoryRecordPool::new(Global);
        let mut element_with_history: ElementWithHistory<(), usize, Global> =
            ElementWithHistory::new((), (), 1, &mut record_pool);

        element_with_history.add_new_record(record_pool.create_record(
            2,
            Some(element_with_history.head),
            CacheSnapshotId(1),
        ));

        element_with_history.rollback(&mut record_pool, CacheSnapshotId(0));
        check_that_head_is_initial_record(1, &element_with_history);
        assert_eq!(element_with_history.committed, element_with_history.initial);
    }

    #[test]
    fn commits_with_initial_value() {
        let mut record_pool = HistoryRecordPool::new(Global);
        let mut element_with_history: ElementWithHistory<(), usize, Global> =
            ElementWithHistory::new((), (), 1, &mut record_pool);

        element_with_history.commit(&mut record_pool);
        check_that_head_is_initial_record(1, &element_with_history);
        assert_eq!(element_with_history.committed, element_with_history.initial);
    }

    #[test]
    fn commits_one_record() {
        let mut record_pool = HistoryRecordPool::new(Global);
        let mut element_with_history: ElementWithHistory<(), usize, Global> =
            ElementWithHistory::new((), (), 1, &mut record_pool);

        let new_record =
            record_pool.create_record(2, Some(element_with_history.head), CacheSnapshotId(1));

        element_with_history.add_new_record(new_record);

        element_with_history.commit(&mut record_pool);
        assert_eq!(element_with_history.head, new_record);
        assert_eq!(element_with_history.first, new_record);
        assert_eq!(element_with_history.committed, new_record);
    }

    #[test]
    fn commits_two_records() {
        let mut record_pool = HistoryRecordPool::new(Global);
        let mut element_with_history: ElementWithHistory<(), usize, Global> =
            ElementWithHistory::new((), (), 1, &mut record_pool);

        let new_record =
            record_pool.create_record(2, Some(element_with_history.head), CacheSnapshotId(1));
        element_with_history.add_new_record(new_record);

        let new_record_2 = record_pool.create_record(3, Some(new_record), CacheSnapshotId(2));
        element_with_history.add_new_record(new_record_2);

        element_with_history.commit(&mut record_pool);

        assert_eq!(element_with_history.head, new_record_2);
        assert_eq!(element_with_history.first, new_record_2);
        assert_eq!(element_with_history.committed, new_record_2);
    }
}

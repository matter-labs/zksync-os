use crate::memory::stack_trait::{StackCtor, StackCtorConst};
use alloc::alloc::Global;
use core::alloc::Allocator;

use super::history_list::HistoryList;

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct HistoryCounterSnapshotId(usize);

impl HistoryCounterSnapshotId {
    pub fn new() -> Self {
        Self(0)
    }
}

pub struct HistoryCounter<
    V,
    SC: StackCtor<SCC>,
    SCC: const StackCtorConst,
    A: Allocator + Clone = Global,
> where
    [(); SCC::extra_const_param::<(V, ()), A>()]:,
{
    history: HistoryList<V, (), SC, SCC, A>,
    last_snapshot_id: HistoryCounterSnapshotId,
}

impl<V, SC: StackCtor<SCC>, SCC: const StackCtorConst, A: Allocator + Clone>
    HistoryCounter<V, SC, SCC, A>
where
    [(); SCC::extra_const_param::<(V, ()), A>()]:,
{
    pub fn new(alloc: A) -> Self {
        Self {
            history: HistoryList::new(alloc),
            last_snapshot_id: HistoryCounterSnapshotId::new(),
        }
    }

    pub fn value(&self) -> Option<&V> {
        self.history.top().map(|(v, _)| v)
    }

    pub fn update(&mut self, value: V) {
        if self.history.len() > self.last_snapshot_id.0 {
            // Just override last record (not snapshotted yet)
            let (v, _) = self.history.top_mut().expect("Should have history records");
            *v = value;
        } else {
            self.history.push(value, ());
        }
    }

    pub fn snapshot(&mut self) -> HistoryCounterSnapshotId {
        self.last_snapshot_id.0 = self.history.snapshot();
        self.last_snapshot_id
    }

    pub fn rollback(&mut self, snapshot: HistoryCounterSnapshotId) {
        self.history.rollback(snapshot.0);
        self.last_snapshot_id = snapshot;
    }
}

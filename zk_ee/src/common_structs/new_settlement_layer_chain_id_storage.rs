use crate::{memory::stack_trait::StackFactory, system::errors::system::SystemError};
use alloc::alloc::Global;
use core::alloc::Allocator;
use ruint::aliases::U256;

use super::history_counter::{HistoryCounter, HistoryCounterSnapshotId};

pub type NewSettlementLayerChainIdSnapshotId = HistoryCounterSnapshotId;

///
/// Storage for updates to the settlement layer chain id.
/// We only care about the latest one, as there should only
/// be at most one such update per batch.
///
pub struct NewSettlementLayerChainIdStorage<
    SF: StackFactory<M>,
    const M: usize,
    A: Allocator + Clone = Global,
> {
    history: HistoryCounter<U256, SF, M, A>,
    _marker: core::marker::PhantomData<A>,
}

impl<SF: StackFactory<M>, const M: usize, A: Allocator + Clone>
    NewSettlementLayerChainIdStorage<SF, M, A>
{
    pub fn new_from_parts(allocator: A) -> Self {
        Self {
            history: HistoryCounter::new(allocator),
            _marker: core::marker::PhantomData,
        }
    }

    #[track_caller]
    pub fn start_frame(&mut self) -> NewSettlementLayerChainIdSnapshotId {
        self.history.snapshot()
    }

    pub fn update(&mut self, new_sl_chain_id: U256) -> Result<(), SystemError> {
        self.history.update(new_sl_chain_id);

        Ok(())
    }

    pub fn value(&self) -> U256 {
        // Zero indicates no updates
        self.history.value().cloned().unwrap_or(U256::ZERO)
    }

    #[track_caller]
    pub fn finish_frame(&mut self, rollback_handle: Option<NewSettlementLayerChainIdSnapshotId>) {
        if let Some(x) = rollback_handle {
            self.history.rollback(x);
        }
    }
}

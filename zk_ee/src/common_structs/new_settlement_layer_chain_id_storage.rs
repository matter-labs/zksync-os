use crate::{
    internal_error, memory::stack_trait::StackFactory, system::errors::system::SystemError,
};
use alloc::alloc::Global;
use core::alloc::Allocator;
use ruint::aliases::U256;

use super::history_counter::{HistoryCounter, HistoryCounterSnapshotId};

pub type NewSettlementLayerChainIdSnapshotId = HistoryCounterSnapshotId;

///
/// Storage for an update to the settlement layer chain id.
/// If the system tries to update it more than once, it will
/// result in an internal error.
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
        if self.value().is_some() {
            return Err(internal_error!(
                "Tried to update settlement layer chain id more than once in a block"
            )
            .into());
        }
        self.history.update(new_sl_chain_id);

        Ok(())
    }

    pub fn value(&self) -> Option<&U256> {
        self.history.value()
    }

    #[track_caller]
    pub fn finish_frame(&mut self, rollback_handle: Option<NewSettlementLayerChainIdSnapshotId>) {
        if let Some(x) = rollback_handle {
            self.history.rollback(x);
        }
    }
}

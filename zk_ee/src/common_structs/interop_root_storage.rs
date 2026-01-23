use crate::{
    memory::stack_trait::StackFactory, system::errors::system::SystemError, utils::Bytes32,
};
use alloc::alloc::Global;
use core::alloc::Allocator;
use ruint::aliases::U256;

use super::history_list::HistoryList;

/// Represents a cross-chain interoperability root that enables
/// communication and state verification between different blockchain networks.
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct InteropRoot {
    /// The merkle root hash (cannot be zero for valid roots)
    pub root: Bytes32,
    /// Block or batch number from the source chain
    pub block_or_batch_number: U256,
    /// Source chain identifier (must be non-zero)
    pub chain_id: U256,
}

pub struct InteropRootStorage<SF: StackFactory<M>, const M: usize, A: Allocator + Clone = Global> {
    list: HistoryList<InteropRoot, (), SF, M, A>,
    _marker: core::marker::PhantomData<A>,
}

impl<SF: StackFactory<M>, const M: usize, A: Allocator + Clone> InteropRootStorage<SF, M, A> {
    pub fn new_from_parts(allocator: A) -> Self {
        Self {
            list: HistoryList::new(allocator),
            _marker: core::marker::PhantomData,
        }
    }

    #[track_caller]
    pub fn start_frame(&mut self) -> usize {
        self.list.snapshot()
    }

    pub fn push_root(&mut self, interop_root: InteropRoot) -> Result<(), SystemError> {
        self.list.push(interop_root, ());

        Ok(())
    }

    #[track_caller]
    pub fn finish_frame(&mut self, rollback_handle: Option<usize>) {
        if let Some(x) = rollback_handle {
            self.list.rollback(x);
        }
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a InteropRoot> {
        self.list.iter()
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }
}

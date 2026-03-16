use crate::utils::Bytes32;

/// Snapshot of a storage slot's state used to interface between
/// the cache and [`crate::common_structs::state_root_view::StateRootView`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct StorageSlotValue {
    pub initial_value: Bytes32,
    pub current_value: Bytes32,
    pub initial_value_used: bool,
    pub is_new_storage_slot: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct TransientStorageValue {
    pub current_value: Bytes32,
    pub changes_stack_depth: usize,
}

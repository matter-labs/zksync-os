use crate::utils::Bytes32;

///
/// Old representation of cache value.
/// For now, only used to interface between history_map and
/// StateRootView.
/// TODO: replace with a simple struct.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct WarmStorageValue {
    pub initial_value: Bytes32,
    pub current_value: Bytes32,
    pub value_at_the_start_of_tx: Bytes32,
    pub changes_stack_depth: usize,
    // None if value hasn't been accessed yet.
    pub last_accessed_at_tx_number: Option<u32>,
    pub pubdata_diff_bytes: u8,
    pub initial_value_used: bool,
    pub is_new_storage_slot: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct TransientStorageValue {
    pub current_value: Bytes32,
    pub changes_stack_depth: usize,
}

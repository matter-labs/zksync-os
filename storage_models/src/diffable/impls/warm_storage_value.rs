use zk_ee::system_io_oracle::InitialStorageSlotData;
use zk_ee::types_config::SystemIOTypesConfig;
use zk_ee::utils::Bytes32;

#[derive(Clone, Copy, Debug)]
pub struct StorageSlotInitializationData {
    pub initial_value: Bytes32,
    pub claimed_is_new_storage_slot_in_implementation: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct StorageSlotWriteRollbackData {
    pub previous_value: Bytes32,
    pub previous_used_tx_number: Option<u32>,
    pub previous_pubdata_diff_bytes: u8,
}

impl<IOTypes: SystemIOTypesConfig<StorageValue = Bytes32>> From<InitialStorageSlotData<IOTypes>>
    for StorageSlotInitializationData
{
    fn from(value: InitialStorageSlotData<IOTypes>) -> Self {
        let init_data = value;
        let InitialStorageSlotData {
            is_new_storage_slot,
            initial_value,
        } = init_data;

        Self {
            initial_value,
            claimed_is_new_storage_slot_in_implementation: is_new_storage_slot,
        }
    }
}

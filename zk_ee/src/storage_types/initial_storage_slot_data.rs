use crate::oracle::word_serialization::{WordDeserializable, WordSerializable};
use crate::types_config::SystemIOTypesConfig;

#[derive(Clone, Copy, Debug, WordSerializable, WordDeserializable)]
pub struct InitialStorageSlotData<IOTypes: SystemIOTypesConfig> {
    // We need to know what was a value of the storage slot,
    // and whether it existed in the state or has to be created
    // (so additional information is needed to reconstruct creation location).
    pub is_new_storage_slot: bool,
    pub initial_value: IOTypes::StorageValue,
}

impl<IOTypes: SystemIOTypesConfig> Default for InitialStorageSlotData<IOTypes> {
    fn default() -> Self {
        Self {
            is_new_storage_slot: false,
            initial_value: IOTypes::StorageValue::default(),
        }
    }
}

use crate::oracle::word_serialization::{WordDeserializable, WordSerializable};
use crate::types_config::SystemIOTypesConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, WordSerializable, WordDeserializable)]
pub struct StorageAddress<IOTypes: SystemIOTypesConfig> {
    pub address: IOTypes::Address,
    pub key: IOTypes::StorageKey,
}

use crate::system::errors::internal::InternalError;
use crate::system::logger::Logger;
use crate::{
    kv_markers::{UsizeDeserializable, UsizeSerializable},
    system_io_oracle::IOOracle,
    types_config::SystemIOTypesConfig,
};
use core::alloc::Allocator;

#[derive(Clone, Copy, Debug)]
pub struct StorageAccessRecord<IOTypes: SystemIOTypesConfig> {
    pub address: IOTypes::Address,
    pub key: IOTypes::StorageKey,
    pub initial_value: IOTypes::StorageValue,
    pub written_value: IOTypes::StorageValue,
    pub is_write: bool,
    pub expected_as_new_in_state: bool,
}

use crate::common_structs::{WarmStorageKey, WarmStorageValue};

///
/// Minimal view of the state root/commitment.
/// We only need to be able to update it by applying and verifying
/// storage updates.
///
pub trait StateRootView<IOTypes: SystemIOTypesConfig>:
    Clone + UsizeSerializable + UsizeDeserializable + core::fmt::Debug
{
    fn verify_and_apply_batch<O: IOOracle, A: Allocator + Clone + Default>(
        &mut self,
        oracle: &mut O,
        source: impl Iterator<Item = (WarmStorageKey, WarmStorageValue)> + Clone,
        allocator: A,
        logger: &mut impl Logger,
    ) -> Result<(), InternalError>;
}

use std::collections::HashMap;

use super::*;
use ruint::aliases::B160;
use zk_ee::{
    oracle::query_ids::INITIAL_STORAGE_SLOT_VALUE_QUERY_ID,
    oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator,
    storage_types::{InitialStorageSlotData, StorageAddress},
};

/// This processor provides a simple HashMap-based implementation for storage
/// queries. It's primarily used for testing or scenarios where the entire
/// storage state can be held in memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InMemoryInitialStorageSlotValueResponder {
    /// Two-level map: address -> (storage_key -> storage_value)
    pub values_map: HashMap<B160, HashMap<Bytes32, Bytes32>>,
}

impl InMemoryInitialStorageSlotValueResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[INITIAL_STORAGE_SLOT_VALUE_QUERY_ID];
}

impl<M: MemorySource> OracleQueryProcessor<M> for InMemoryInitialStorageSlotValueResponder {
    fn supported_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        Self::SUPPORTED_QUERY_IDS.contains(&query_id)
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        _memory: &M,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        let address = StorageAddress::<EthereumIOTypesConfig>::from_iter(&mut query.into_iter())
            .expect("must deserialize hash value");

        let value = if let Some(storage) = self.values_map.get(&address.address) {
            storage.get(&address.key).copied().unwrap_or(Bytes32::ZERO)
        } else {
            Bytes32::ZERO
        };

        let is_new = value.is_zero();
        let initial_value = InitialStorageSlotData::<EthereumIOTypesConfig> {
            is_new_storage_slot: is_new,
            initial_value: value,
        };

        DynUsizeIterator::from_constructor(initial_value, |inner_ref| {
            UsizeSerializable::iter(inner_ref)
        })
    }
}

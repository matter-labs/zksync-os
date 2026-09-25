use super::*;
use crate::run::ReadStorage;
use zk_ee::common_structs::derive_flat_storage_key;
use zk_ee::storage_types::InitialStorageSlotData;

/// This processor handles requests for reading initial storage slot values
/// from the storage layer. It duplicates the storage read functionality of ReadTreeResponder
/// without additional tree operations and validations. This is useful for simulations.
#[derive(Clone, Debug)]
pub struct ReadStorageResponder<S: ReadStorage> {
    pub storage: S,
}

impl<S: ReadStorage> ReadStorageResponder<S> {
    const SUPPORTED_QUERY_IDS: &[u32] =
        &[InitialStorageSlotQuery::<EthereumIOTypesConfig>::QUERY_ID];
}

impl<S: ReadStorage> OracleQueryProcessor for ReadStorageResponder<S> {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![]
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        _query: Vec<usize>,
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        unreachable!("query 0x{query_id:08x} is served with the memory-based protocol")
    }

    fn supported_memory_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn process_memory_query(
        &mut self,
        query_id: u32,
        input_word: usize,
        memory: &dyn QuerierMemory,
    ) -> Vec<u32> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        let (address, key) = read_storage_slot_query(memory, input_word);
        let flat_key = derive_flat_storage_key(&address, &key);
        let slot_data: InitialStorageSlotData<EthereumIOTypesConfig> =
            if let Some(cold) = self.storage.read(flat_key) {
                InitialStorageSlotData {
                    initial_value: cold,
                    is_new_storage_slot: false,
                }
            } else {
                // default value, but it's potentially new storage slot in state!
                InitialStorageSlotData {
                    initial_value: Bytes32::ZERO,
                    is_new_storage_slot: true,
                }
            };
        memory_response(&slot_data)
    }
}

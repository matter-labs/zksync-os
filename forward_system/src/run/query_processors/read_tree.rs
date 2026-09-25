use super::*;
use crate::run::ReadStorageTree;
use basic_system::system_implementation::flat_storage_model::{
    write_proof_for_index_response, ExactIndexQuery, PreviousIndexQuery, PROOF_FOR_INDEX_QUERY_ID,
};
use zk_ee::common_structs::derive_flat_storage_key;
use zk_ee::storage_types::InitialStorageSlotData;

/// This processor handles requests related to the storage tree structure,
/// including storage slot reads (similar to ReadStorageResponder), tree index
/// lookups, and Merkle proof generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadTreeResponder<T: ReadStorageTree> {
    pub tree: T,
}

impl<T: ReadStorageTree> ReadTreeResponder<T> {
    /// # Query Types
    /// - `PreviousIndexQuery`: Returns the previous tree index for a given key
    /// - `ExactIndexQuery`: Returns the exact tree index for a key (panics if not found)
    /// - `InitialStorageSlotQuery`: Returns storage slot data and metadata
    /// - `ProofForIndexQuery`: Returns Merkle proof for a tree index
    const SUPPORTED_MEMORY_QUERY_IDS: &[u32] = &[
        InitialStorageSlotQuery::<EthereumIOTypesConfig>::QUERY_ID,
        PreviousIndexQuery::QUERY_ID,
        ExactIndexQuery::QUERY_ID,
        PROOF_FOR_INDEX_QUERY_ID,
    ];
}

impl<T: ReadStorageTree> OracleQueryProcessor for ReadTreeResponder<T> {
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
        Self::SUPPORTED_MEMORY_QUERY_IDS.to_vec()
    }

    fn process_memory_query(
        &mut self,
        query_id: u32,
        input_word: usize,
        memory: &dyn QuerierMemory,
    ) -> Vec<u32> {
        assert!(Self::SUPPORTED_MEMORY_QUERY_IDS.contains(&query_id));

        match query_id {
            PreviousIndexQuery::QUERY_ID => {
                let key = Bytes32::read_input(memory, input_word).expect("must read key");
                memory_response(&self.tree.prev_tree_index(key))
            }
            ExactIndexQuery::QUERY_ID => {
                let key = Bytes32::read_input(memory, input_word).expect("must read key");
                let existing = self
                    .tree
                    .tree_index(key)
                    .expect("Reading index for key that is not in the tree");
                memory_response(&existing)
            }
            InitialStorageSlotQuery::<EthereumIOTypesConfig>::QUERY_ID => {
                let (address, key) = read_storage_slot_query(memory, input_word);
                let flat_key = derive_flat_storage_key(&address, &key);
                let slot_data: InitialStorageSlotData<EthereumIOTypesConfig> =
                    if let Some(cold) = self.tree.read(flat_key) {
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
            PROOF_FOR_INDEX_QUERY_ID => {
                let index = u64::read_input(memory, input_word).expect("must read index");
                let mut response = Vec::new();
                write_proof_for_index_response(&self.tree.merkle_proof(index), &mut response);
                response
            }
            _ => unreachable!(),
        }
    }
}

use super::*;
use crate::run::ReadStorageTree;
use basic_system::system_implementation::flat_storage_model::*;
use basic_system::system_implementation::flat_storage_model::{
    ExactIndexQuery, PreviousIndexQuery, PROOF_FOR_INDEX_QUERY_ID,
};
use zk_ee::common_structs::derive_flat_storage_key;
use zk_ee::oracle::simple_oracle_query::SimpleOracleQuery;
use zk_ee::storage_types::InitialStorageSlotData;
use zk_ee::storage_types::StorageAddress;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::{oracle::basic_queries::InitialStorageSlotQuery, utils::Bytes32};

/// This processor handles requests related to the storage tree structure,
/// including storage slot reads (similar to ReadStorageResponder), tree index
/// lookups, and Merkle proof generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadTreeResponder<T: ReadStorageTree> {
    pub tree: T,
}

/// Decode WordLayout-encoded u32 words back into a typed value.
fn decode_input<T: WordLayout>(input: &[u32]) -> T {
    let mut cursor = 0;
    T::read_words(&mut || {
        let w = input.get(cursor).copied().unwrap_or(0);
        cursor += 1;
        w
    })
}

impl<T: ReadStorageTree> ReadTreeResponder<T> {
    /// # Query Types
    /// - `PreviousIndexQuery`: Returns the previous tree index for a given key
    /// - `ExactIndexQuery`: Returns the exact tree index for a key (panics if not found)
    /// - `InitialStorageSlotQuery`: Returns storage slot data and metadata
    /// - `PROOF_FOR_INDEX_QUERY_ID`: Returns Merkle proof for a tree index
    const SUPPORTED_QUERY_IDS: &[u32] = &[
        InitialStorageSlotQuery::<EthereumIOTypesConfig>::QUERY_ID,
        PreviousIndexQuery::QUERY_ID,
        ExactIndexQuery::QUERY_ID,
        PROOF_FOR_INDEX_QUERY_ID,
    ];
}

impl<T: ReadStorageTree> OracleQueryProcessor for ReadTreeResponder<T> {
    fn supported_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        Self::SUPPORTED_QUERY_IDS.contains(&query_id)
    }

    fn process(
        &mut self,
        query_id: u32,
        input: &[u32],
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Result<Vec<u32>, InternalError> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        let mut result = Vec::new();
        match query_id {
            PreviousIndexQuery::QUERY_ID => {
                let key: <PreviousIndexQuery as SimpleOracleQuery>::Input = decode_input(input);
                let prev_index = self.tree.prev_tree_index(key);
                prev_index.write_words(&mut |w| result.push(w));
            }
            ExactIndexQuery::QUERY_ID => {
                let key: <ExactIndexQuery as SimpleOracleQuery>::Input = decode_input(input);
                let existing = self
                    .tree
                    .tree_index(key)
                    .expect("Reading index for key that is not in the tree");
                existing.write_words(&mut |w| result.push(w));
            }
            InitialStorageSlotQuery::<EthereumIOTypesConfig>::QUERY_ID => {
                let storage_addr: StorageAddress<EthereumIOTypesConfig> = decode_input(input);
                let flat_key = derive_flat_storage_key(&storage_addr.address, &storage_addr.key);
                let slot_data: InitialStorageSlotData<EthereumIOTypesConfig> =
                    if let Some(cold) = self.tree.read(flat_key) {
                        InitialStorageSlotData {
                            initial_value: cold,
                            is_new_storage_slot: false,
                        }
                    } else {
                        InitialStorageSlotData {
                            initial_value: Bytes32::ZERO,
                            is_new_storage_slot: true,
                        }
                    };
                slot_data.write_words(&mut |w| result.push(w));
            }
            PROOF_FOR_INDEX_QUERY_ID => {
                let index: u64 = decode_input(input);
                let existing = self.tree.merkle_proof(index);
                let proof = ValueAtIndexProof {
                    proof: ExistingReadProof { existing },
                };
                proof.write_words(&mut |w| result.push(w));
            }
            _ => unreachable!(),
        }
        Ok(result)
    }
}

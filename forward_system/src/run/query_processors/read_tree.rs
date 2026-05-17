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
        input: &[u8],
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Result<Vec<u8>, InternalError> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        match query_id {
            PreviousIndexQuery::QUERY_ID => {
                let key: <PreviousIndexQuery as SimpleOracleQuery>::Input =
                    AirbenderCodecV0::decode(input)
                        .map_err(|_| internal_error!("decode PreviousIndexQuery input failed"))?;
                let prev_index = self.tree.prev_tree_index(key);
                AirbenderCodecV0::encode(&prev_index)
                    .map_err(|_| internal_error!("encode prev_index failed"))
            }
            ExactIndexQuery::QUERY_ID => {
                let key: <ExactIndexQuery as SimpleOracleQuery>::Input =
                    AirbenderCodecV0::decode(input)
                        .map_err(|_| internal_error!("decode ExactIndexQuery input failed"))?;
                let existing = self
                    .tree
                    .tree_index(key)
                    .expect("Reading index for key that is not in the tree");
                AirbenderCodecV0::encode(&existing)
                    .map_err(|_| internal_error!("encode tree index failed"))
            }
            InitialStorageSlotQuery::<EthereumIOTypesConfig>::QUERY_ID => {
                let storage_address: StorageAddress<EthereumIOTypesConfig> =
                    AirbenderCodecV0::decode(input)
                        .map_err(|_| internal_error!("decode StorageAddress failed"))?;
                let flat_key =
                    derive_flat_storage_key(&storage_address.address, &storage_address.key);
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
                AirbenderCodecV0::encode(&slot_data)
                    .map_err(|_| internal_error!("encode slot_data failed"))
            }
            PROOF_FOR_INDEX_QUERY_ID => {
                let index: u64 = AirbenderCodecV0::decode(input)
                    .map_err(|_| internal_error!("decode proof index failed"))?;
                let existing = self.tree.merkle_proof(index);
                let proof = ValueAtIndexProof {
                    proof: ExistingReadProof { existing },
                };
                AirbenderCodecV0::encode(&proof).map_err(|_| internal_error!("encode proof failed"))
            }
            _ => unreachable!(),
        }
    }
}

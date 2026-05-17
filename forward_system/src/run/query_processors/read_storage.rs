use super::*;
use crate::run::ReadStorage;
use zk_ee::common_structs::derive_flat_storage_key;
use zk_ee::oracle::simple_oracle_query::SimpleOracleQuery;
use zk_ee::storage_types::InitialStorageSlotData;
use zk_ee::storage_types::StorageAddress;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::{oracle::basic_queries::InitialStorageSlotQuery, utils::Bytes32};

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
            InitialStorageSlotQuery::<EthereumIOTypesConfig>::QUERY_ID => {
                let storage_address: StorageAddress<EthereumIOTypesConfig> =
                    AirbenderCodecV0::decode(input)
                        .map_err(|_| internal_error!("decode StorageAddress failed"))?;
                let flat_key =
                    derive_flat_storage_key(&storage_address.address, &storage_address.key);
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
                AirbenderCodecV0::encode(&slot_data)
                    .map_err(|_| internal_error!("encode slot_data failed"))
            }
            _ => unreachable!(),
        }
    }
}

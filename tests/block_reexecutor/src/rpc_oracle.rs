use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::rpc_client::RpcClient;

use alloy::hex;
use rig::basic_system::system_implementation::flat_storage_model::{
    FlatStorageCommitment, FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID, TREE_HEIGHT,
};
use rig::chain::TestingOracleFactory;
use rig::forward_system::run::query_processors::{
    BlockMetadataResponder, DACommitmentSchemeResponder, TxDataResponder, ZKProofDataResponder,
};
use rig::forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree};
use rig::oracle_provider::{
    DummyMemorySource, MemorySource, OracleQueryProcessor, ZkEENonDeterminismSource,
};
use rig::risc_v_simulator::abstractions::memory::VectorMemoryImpl;
use rig::zk_ee::common_structs::{da_commitment_scheme::DACommitmentScheme, ProofData};
use rig::zk_ee::oracle::basic_queries::InitialStorageSlotQuery;
use rig::zk_ee::oracle::simple_oracle_query::SimpleOracleQuery;
use rig::zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;
use rig::zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use rig::zk_ee::storage_types::{InitialStorageSlotData, StorageAddress};
use rig::zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle;
use rig::zk_ee::types_config::EthereumIOTypesConfig;
use rig::zk_ee::utils::usize_rw::ReadIterWrapper;
use rig::zk_ee::utils::Bytes32;
use rig::zksync_os_api;
use rig::zksync_os_interface::traits::TxListSource;
use ruint::aliases::{B160, U256};
use ruint::Bits;

use rig::basic_system::system_implementation::flat_storage_model::ACCOUNT_PROPERTIES_STORAGE_ADDRESS;

/// Oracle responder that fetches initial storage slot values via RPC
struct RpcStorageResponder {
    client: RpcClient,
    /// Block number whose pre-state we query (i.e. state *before* this block).
    /// RPC calls use `block_number - 1` as the block tag.
    prev_block_number: u64,
    /// Cache to avoid repeated RPC calls for the same slot
    cache: Arc<Mutex<HashMap<(Bits<160, 3>, Bytes32), Bytes32>>>,
    preimages: Arc<Mutex<HashMap<Bytes32, Vec<u8>>>>,
}

impl RpcStorageResponder {
    pub fn new(
        endpoint: String,
        prev_block_number: u64,
        cache: Arc<Mutex<HashMap<(Bits<160, 3>, Bytes32), Bytes32>>>,
        preimages: Arc<Mutex<HashMap<Bytes32, Vec<u8>>>>,
    ) -> Self {
        Self {
            client: RpcClient::new(endpoint),
            prev_block_number,
            cache,
            preimages,
        }
    }

    const SUPPORTED_QUERY_IDS: &[u32] = &[
        InitialStorageSlotQuery::<EthereumIOTypesConfig>::QUERY_ID,
        FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID,
    ];

    pub fn set_account_properties(
        preimages_cache: &Arc<Mutex<HashMap<Bytes32, Vec<u8>>>>,
        address: B160,
        balance: U256,
        nonce: u64,
        bytecode: Option<Vec<u8>>,
    ) -> Bytes32 {
        use zksync_os_api::helpers::*;
        let mut account_properties = Default::default();

        let mut preimages = preimages_cache
            .lock()
            .expect("Failed to lock preimages cache");
        if let Some(bytecode) = bytecode {
            let bytecode_and_artifacts = set_properties_code(&mut account_properties, &bytecode);
            preimages.insert(account_properties.bytecode_hash, bytecode_and_artifacts);
        }
        println!(
            "RPC set account properties: address={:?}, balance={}, nonce={}",
            address, balance, nonce
        );

        set_properties_balance(&mut account_properties, balance);
        set_properties_nonce(&mut account_properties, nonce);

        let encoding = account_properties.encoding();
        let properties_hash = account_properties.compute_hash();

        preimages.insert(properties_hash, encoding.to_vec());

        properties_hash
    }
}

impl<M: MemorySource> OracleQueryProcessor<M> for RpcStorageResponder {
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
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));
        match query_id {
            InitialStorageSlotQuery::<EthereumIOTypesConfig>::QUERY_ID => {
                let StorageAddress { address, key } = <InitialStorageSlotQuery<
                    EthereumIOTypesConfig,
                > as SimpleOracleQuery>::Input::from_iter(
                    &mut query.into_iter()
                )
                .expect("must deserialize the address/slot");

                let mut cached_value = self
                    .cache
                    .lock()
                    .expect("Failed to lock storage cache")
                    .get(&(address, key))
                    .copied();

                if cached_value.is_none() && address == ACCOUNT_PROPERTIES_STORAGE_ADDRESS {
                    let requested_address: Bits<160, 3> = key.into();

                    let balance = self
                        .client
                        .get_balance(
                            requested_address.to_be_bytes().into(),
                            self.prev_block_number,
                        )
                        .expect("RPC balance fetch failed");

                    let nonce = self
                        .client
                        .get_transaction_count(
                            requested_address.to_be_bytes().into(),
                            self.prev_block_number,
                        )
                        .expect("RPC nonce fetch failed");

                    let bytecode = self
                        .client
                        .get_code(
                            requested_address.to_be_bytes().into(),
                            self.prev_block_number,
                        )
                        .expect("RPC code fetch failed");

                    let hash = Self::set_account_properties(
                        &self.preimages,
                        requested_address,
                        balance,
                        nonce,
                        if bytecode.is_empty() {
                            None
                        } else {
                            Some(bytecode.to_vec())
                        },
                    );

                    self.cache
                        .lock()
                        .expect("Failed to lock storage cache")
                        .insert((address, key), hash);
                    cached_value = Some(hash);
                }

                let slot_data: InitialStorageSlotData<EthereumIOTypesConfig> =
                    if let Some(cold) = cached_value {
                        InitialStorageSlotData {
                            initial_value: cold,
                            is_new_storage_slot: cold.is_zero(),
                        }
                    } else {
                        let value = self
                            .client
                            .get_storage_at(
                                address.to_be_bytes().into(),
                                key.into_u256_be(),
                                self.prev_block_number,
                            )
                            .expect("RPC storage fetch failed");
                        let bytes32_value = Bytes32::from_u256_be(&value);
                        self.cache
                            .lock()
                            .expect("Failed to lock storage cache")
                            .insert((address, key), bytes32_value);
                        InitialStorageSlotData {
                            initial_value: bytes32_value,
                            is_new_storage_slot: value.is_zero(),
                        }
                    };

                DynUsizeIterator::from_constructor(slot_data, UsizeSerializable::iter)
            }
            FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID => {
                let hash = Bytes32::from_iter(&mut query.into_iter())
                    .expect("must deserialize hash value");

                let preimage = self
                    .preimages
                    .lock()
                    .expect("Failed to lock preimages cache")
                    .get(&hash)
                    .cloned()
                    .unwrap_or_else(|| {
                        panic!(
                            "must know a preimage for hash {} for query ID 0x{:016x}",
                            hex::encode(hash.as_u8_array_ref()),
                            query_id
                        )
                    });

                DynUsizeIterator::from_constructor(preimage, |inner_ref| {
                    ReadIterWrapper::from(inner_ref.iter().copied())
                })
            }
            _ => unreachable!(),
        }
    }
}

pub struct RpcValueOracleFactory {
    endpoint: String,
    prev_block_number: u64,
    pub cache: Arc<Mutex<HashMap<(Bits<160, 3>, Bytes32), Bytes32>>>,
    pub preimages: Arc<Mutex<HashMap<Bytes32, Vec<u8>>>>,
}

impl RpcValueOracleFactory {
    pub fn new(endpoint: String, block_number: u64) -> Self {
        Self {
            endpoint,
            prev_block_number: block_number.saturating_sub(1),
            cache: Arc::new(Mutex::new(HashMap::new())),
            preimages: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl TestingOracleFactory<false> for RpcValueOracleFactory {
    fn create_forward_oracle(
        &self,
        block_metadata: BlockMetadataFromOracle,
        _state_tree: InMemoryTree<false>,
        _preimage_source: InMemoryPreimageSource,
        tx_source: TxListSource,
        proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        _add_uart: bool,
        _use_native_callable_oracles: bool,
    ) -> ZkEENonDeterminismSource<DummyMemorySource> {
        let block_metadata_responder = BlockMetadataResponder { block_metadata };
        let tx_data_responder = TxDataResponder {
            tx_source,
            next_tx: None,
            next_tx_format: None,
            next_tx_from: None,
        };

        let storage_responder = RpcStorageResponder::new(
            self.endpoint.clone(),
            self.prev_block_number,
            self.cache.clone(),
            self.preimages.clone(),
        );

        let zk_proof_data_responder = ZKProofDataResponder { data: proof_data };

        let da_commitment_scheme_responder = DACommitmentSchemeResponder {
            da_commitment_scheme,
        };

        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(block_metadata_responder);
        oracle.add_external_processor(tx_data_responder);
        oracle.add_external_processor(storage_responder);
        oracle.add_external_processor(zk_proof_data_responder);
        oracle.add_external_processor(da_commitment_scheme_responder);

        oracle
    }

    fn create_proof_oracle(
        &self,
        _block_metadata: BlockMetadataFromOracle,
        _state_tree: InMemoryTree<false>,
        _preimage_source: InMemoryPreimageSource,
        _tx_source: TxListSource,
        _proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
        _da_commitment_scheme: Option<DACommitmentScheme>,
        _add_uart: bool,
        _use_native_callable_oracles: bool,
    ) -> ZkEENonDeterminismSource<VectorMemoryImpl> {
        // Note: block reexecutor does not use proof oracle
        ZkEENonDeterminismSource::default()
    }
}

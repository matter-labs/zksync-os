#![cfg(test)]

//!
//! Regression test for the initial storage slot value assertion fix.
//!
//! This test verifies that when reading new (empty) storage slots, the initial value
//! must be Bytes32::ZERO, preventing bugs where invalid initial values might be provided.
//!

use rig::alloy::consensus::TxEip2930;
use rig::alloy::primitives::{address, TxKind, U256};
use rig::basic_system::system_implementation::flat_storage_model::{
    FlatStorageCommitment, TREE_HEIGHT,
};
use rig::chain::TestingOracleFactory;
use rig::forward_system::run::convert_alloy::FromAlloy;
use rig::forward_system::run::query_processors::{
    BlockMetadataResponder, DACommitmentSchemeResponder, GenericPreimageResponder, TxDataResponder,
    ZKProofDataResponder,
};
use rig::forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree};
use rig::forward_system::run::ReadStorage;
use rig::oracle_provider::{
    DummyMemorySource, MemorySource, OracleQueryProcessor, ZkEENonDeterminismSource,
};
use rig::risc_v_simulator::abstractions::memory::VectorMemoryImpl;
use rig::ruint::aliases::B160;
use rig::zk_ee::common_structs::{
    da_commitment_scheme::DACommitmentScheme, derive_flat_storage_key, ProofData,
};
use rig::zk_ee::oracle::basic_queries::InitialStorageSlotQuery;
use rig::zk_ee::oracle::simple_oracle_query::SimpleOracleQuery;
use rig::zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;
use rig::zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use rig::zk_ee::storage_types::{InitialStorageSlotData, StorageAddress};
use rig::zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle;
use rig::zk_ee::types_config::EthereumIOTypesConfig;
use rig::zk_ee::utils::Bytes32;
use rig::zksync_os_interface::traits::TxListSource;
use rig::TestingFramework;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

/// Malicious storage responder that returns non-zero initial values for new storage slots
#[derive(Clone, Debug)]
struct MaliciousStorageResponder<S: ReadStorage> {
    storage: S,
    targets: Vec<(B160, Bytes32)>, // (address, slot)
}

impl<S: ReadStorage> MaliciousStorageResponder<S> {
    fn new(storage: S, targets: Vec<(B160, Bytes32)>) -> Self {
        Self { storage, targets }
    }

    const SUPPORTED_QUERY_IDS: &[u32] =
        &[InitialStorageSlotQuery::<EthereumIOTypesConfig>::QUERY_ID];
}

impl<S: ReadStorage, M: MemorySource> OracleQueryProcessor<M> for MaliciousStorageResponder<S> {
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
                let flat_key = derive_flat_storage_key(&address, &key);
                let slot_data: InitialStorageSlotData<EthereumIOTypesConfig> =
                    if let Some(cold) = self.storage.read(flat_key) {
                        InitialStorageSlotData {
                            initial_value: cold,
                            is_new_storage_slot: false,
                        }
                    } else {
                        let should_use_invalid_value =
                            self.targets.iter().any(|(target_address, target_key)| {
                                *target_address == address && *target_key == key
                            });

                        if should_use_invalid_value {
                            // MALICIOUS: Return a non-zero initial value for new storage slots
                            // This should trigger the assertion in the flat storage model
                            InitialStorageSlotData {
                                initial_value: Bytes32::from_array([42; 32]), // Invalid non-zero value
                                is_new_storage_slot: true,
                            }
                        } else {
                            // Return correct default value
                            InitialStorageSlotData {
                                initial_value: Bytes32::from_array([0; 32]),
                                is_new_storage_slot: true,
                            }
                        }
                    };
                DynUsizeIterator::from_constructor(slot_data, UsizeSerializable::iter)
            }
            _ => unreachable!(),
        }
    }
}

/// Custom oracle factory that injects invalid initial values for storage reads
/// to trigger the assertion in the flat storage model
struct InvalidInitialValueOracleFactory {
    targets: Vec<(B160, Bytes32)>, // (address, slot)
}

impl InvalidInitialValueOracleFactory {
    fn new(targets: Vec<(B160, Bytes32)>) -> Self {
        Self { targets }
    }

    fn build_oracle<M: MemorySource + 'static>(
        &self,
        block_metadata: BlockMetadataFromOracle,
        state_tree: InMemoryTree<false>,
        preimage_source: InMemoryPreimageSource,
        tx_source: TxListSource,
        proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
        da_commitment_scheme: Option<DACommitmentScheme>,
    ) -> ZkEENonDeterminismSource<M> {
        // Create a malicious oracle manually instead of using the default factory
        let block_metadata_responder = BlockMetadataResponder { block_metadata };
        let tx_data_responder = TxDataResponder {
            tx_source,
            next_tx: None,
            next_tx_format: None,
            next_tx_from: None,
        };
        let preimage_responder = GenericPreimageResponder { preimage_source };

        // Use the malicious storage responder instead of the tree responder
        let malicious_storage_responder =
            MaliciousStorageResponder::new(state_tree, self.targets.clone());

        let zk_proof_data_responder = ZKProofDataResponder { data: proof_data };

        let da_commitment_scheme_responder = DACommitmentSchemeResponder {
            da_commitment_scheme: da_commitment_scheme,
        };

        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(block_metadata_responder);
        oracle.add_external_processor(tx_data_responder);
        oracle.add_external_processor(preimage_responder);
        oracle.add_external_processor(malicious_storage_responder);
        oracle.add_external_processor(zk_proof_data_responder);
        oracle.add_external_processor(da_commitment_scheme_responder);

        oracle
    }
}

impl TestingOracleFactory<false> for InvalidInitialValueOracleFactory {
    fn create_forward_oracle(
        &self,
        block_metadata: BlockMetadataFromOracle,
        state_tree: InMemoryTree<false>,
        preimage_source: InMemoryPreimageSource,
        tx_source: TxListSource,
        proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        _add_uart: bool,
        _use_native_callable_oracles: bool,
    ) -> ZkEENonDeterminismSource<DummyMemorySource> {
        self.build_oracle(
            block_metadata,
            state_tree,
            preimage_source,
            tx_source,
            proof_data,
            da_commitment_scheme,
        )
    }

    fn create_proof_oracle(
        &self,
        block_metadata: BlockMetadataFromOracle,
        state_tree: InMemoryTree<false>,
        preimage_source: InMemoryPreimageSource,
        tx_source: TxListSource,
        proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        _add_uart: bool,
        _use_native_callable_oracles: bool,
    ) -> ZkEENonDeterminismSource<VectorMemoryImpl> {
        self.build_oracle(
            block_metadata,
            state_tree,
            preimage_source,
            tx_source,
            proof_data,
            da_commitment_scheme,
        )
    }
}

#[test]
#[should_panic(expected = "Initial value of empty slot must be trivial")]
fn test_initial_slot_value_assertion() {
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();

    let contract_address = address!("1000000000000000000000000000000000000001");

    // Simple storage contract bytecode that implements:
    // function store(uint256 value) { data = value; }
    let simple_storage_bytecode = hex::decode("6080604052348015600e575f5ffd5b50600436106026575f3560e01c80636057361d14602a575b5f5ffd5b60406004803603810190603c9190607d565b6042565b005b805f8190555050565b5f5ffd5b5f819050919050565b605f81604f565b81146068575f5ffd5b50565b5f813590506077816058565b92915050565b5f60208284031215608f57608e604b565b5b5f609a84828501606b565b9150509291505056fea26469706673582212209a9900f35fcdc7903c2ece72cf1b055b4dda7395c3555e100df93ef7977e707064736f6c634300081e0033").unwrap();

    // Create a transaction that calls store(42) which writes to storage slot 0
    // Function selector for store(uint256): 6057361d
    let calldata =
        hex::decode("6057361d000000000000000000000000000000000000000000000000000000000000002a")
            .unwrap();

    let tx = {
        let tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 75_000,
            to: TxKind::Call(contract_address),
            value: Default::default(),
            input: calldata.into(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    // Use the malicious oracle factory that should trigger the assertion
    let malicious_factory = InvalidInitialValueOracleFactory::new(vec![(
        B160::from_alloy(contract_address),
        Bytes32::zero(),
    )]);

    tester = tester
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64))
        .with_evm_contract(contract_address, &simple_storage_bytecode)
        .with_custom_oracle_factory(malicious_factory);

    // This should panic with "initial value of empty slot must be trivial"
    // when the oracle returns invalid initial values for empty storage slots
    let _result = tester.execute_block(vec![tx]);
}

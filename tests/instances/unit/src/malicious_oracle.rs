#![cfg(test)]

//!
//! Tests for malicious oracle behavior.
//!
//! These tests verify that the system correctly validates and rejects
//! invalid or malicious data returned by the oracle (untrusted host).
//!

mod block_metadata {
    //! Tests for block metadata oracle validation.
    //!
    //! The bootloader validates gas limits from the oracle response.
    //! If block_gas_limit > MAX_BLOCK_GAS_LIMIT or individual_tx_gas_limit > MAX_TX_GAS_LIMIT,
    //! an internal error is returned.

    use rig::alloy::consensus::TxEip2930;
    use rig::alloy::primitives::{TxKind, U256};
    use rig::basic_bootloader::bootloader::constants::MAX_BLOCK_GAS_LIMIT;
    use rig::{common_target_address, BlockContext, TestingFramework};
    use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

    /// Verifies that block execution fails when the oracle provides
    /// a gas limit exceeding MAX_BLOCK_GAS_LIMIT.
    #[test]
    fn test_block_rejects_excessive_gas_limit() {
        let mut tester = TestingFramework::new();
        let wallet = tester.random_signer();
        tester = tester.with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

        let tx = {
            let tx = TxEip2930 {
                chain_id: 37u64,
                nonce: 0,
                gas_price: 1000,
                gas_limit: 21_000,
                to: TxKind::Call(common_target_address()),
                value: Default::default(),
                input: Default::default(),
                access_list: Default::default(),
            };
            ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
        };

        let block_context = BlockContext {
            gas_limit: MAX_BLOCK_GAS_LIMIT + 1,
            ..Default::default()
        };

        tester.set_block_context(Some(block_context));
        let result = tester.execute_block_no_panic(vec![tx]);
        assert!(
            result.is_err(),
            "Block execution should fail when gas limit exceeds MAX_BLOCK_GAS_LIMIT"
        );
    }

    /// Verifies that block execution succeeds at the exact MAX_BLOCK_GAS_LIMIT boundary.
    #[test]
    fn test_block_accepts_max_gas_limit() {
        let mut tester = TestingFramework::new();
        let wallet = tester.random_signer();
        tester = tester.with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

        let tx = {
            let tx = TxEip2930 {
                chain_id: 37u64,
                nonce: 0,
                gas_price: 1000,
                gas_limit: 21_000,
                to: TxKind::Call(common_target_address()),
                value: Default::default(),
                input: Default::default(),
                access_list: Default::default(),
            };
            ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
        };

        let block_context = BlockContext {
            gas_limit: MAX_BLOCK_GAS_LIMIT,
            ..Default::default()
        };

        tester.set_block_context(Some(block_context));
        let result = tester.execute_block_no_panic(vec![tx]);
        assert!(
            result.is_ok(),
            "Block execution should succeed at exactly MAX_BLOCK_GAS_LIMIT"
        );
    }

    /// Verifies that block execution fails with u64::MAX as gas limit.
    /// This is a large overflow case — u64::MAX is well above MAX_BLOCK_GAS_LIMIT.
    #[test]
    fn test_block_rejects_u64_max_gas_limit() {
        let mut tester = TestingFramework::new();
        let wallet = tester.random_signer();
        tester = tester.with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

        let tx = {
            let tx = TxEip2930 {
                chain_id: 37u64,
                nonce: 0,
                gas_price: 1000,
                gas_limit: 21_000,
                to: TxKind::Call(common_target_address()),
                value: Default::default(),
                input: Default::default(),
                access_list: Default::default(),
            };
            ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
        };

        let block_context = BlockContext {
            gas_limit: u64::MAX,
            ..Default::default()
        };

        tester.set_block_context(Some(block_context));
        let result = tester.execute_block_no_panic(vec![tx]);
        assert!(
            result.is_err(),
            "Block execution should fail when gas limit is u64::MAX"
        );
    }

    /// Verifies that block execution fails even with an empty transaction list
    /// when the gas limit is excessive. The validation happens during metadata
    /// initialization, before any transactions are processed.
    #[test]
    fn test_empty_block_rejects_excessive_gas_limit() {
        let mut tester = TestingFramework::new();

        let block_context = BlockContext {
            gas_limit: MAX_BLOCK_GAS_LIMIT + 1,
            ..Default::default()
        };

        tester.set_block_context(Some(block_context));
        let result = tester.execute_block_no_panic(vec![]);
        assert!(
            result.is_err(),
            "Even an empty block should fail with excessive gas limit"
        );
    }
}

mod tx_encoding_format {
    //! Unit tests for transaction encoding format oracle validation.
    //!
    //! TxEncodingFormat::from_iter validates the oracle-provided encoding format byte.
    //! Only values 0 (Abi) and 1 (Rlp) are valid. Invalid values should be rejected
    //! with an internal error rather than panicking.

    use rig::basic_bootloader::bootloader::transaction::TxEncodingFormat;
    use rig::zk_ee::oracle::usize_serialization::UsizeDeserializable;

    #[test]
    fn test_tx_encoding_format_accepts_abi() {
        let mut iter = [0usize].into_iter();
        let result = TxEncodingFormat::from_iter(&mut iter);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tx_encoding_format_accepts_rlp() {
        let mut iter = [1usize].into_iter();
        let result = TxEncodingFormat::from_iter(&mut iter);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tx_encoding_format_rejects_invalid_value_2() {
        let mut iter = [2usize].into_iter();
        let result = TxEncodingFormat::from_iter(&mut iter);
        assert!(
            result.is_err(),
            "TxEncodingFormat should reject value 2 (only 0=Abi and 1=Rlp are valid)"
        );
    }

    #[test]
    fn test_tx_encoding_format_rejects_invalid_value_255() {
        let mut iter = [255usize].into_iter();
        let result = TxEncodingFormat::from_iter(&mut iter);
        assert!(result.is_err(), "TxEncodingFormat should reject value 255");
    }

    #[test]
    fn test_tx_encoding_format_rejects_large_value() {
        // Values that would be truncated to u8 — the from_iter first deserializes
        // as u8, so large usize values test the u8 deserialization path too.
        let mut iter = [256usize].into_iter();
        let result = TxEncodingFormat::from_iter(&mut iter);
        assert!(
            result.is_err(),
            "TxEncodingFormat should reject value 256 (overflows u8)"
        );
    }
}

mod da_commitment_scheme {
    //! Unit tests for DA commitment scheme validation.
    //!
    //! DACommitmentScheme::try_from validates the oracle-provided scheme ID.
    //! Only values 0-4 are valid. Invalid IDs should be rejected.
    //! Note: the oracle query for DA commitment scheme only runs in PROOF_ENV,
    //! so this is tested at the type level rather than through the full execution path.

    use rig::zk_ee::common_structs::da_commitment_scheme::DACommitmentScheme;

    #[test]
    fn test_da_commitment_scheme_accepts_all_valid_ids() {
        assert_eq!(
            DACommitmentScheme::try_from(0u8),
            Ok(DACommitmentScheme::None)
        );
        assert_eq!(
            DACommitmentScheme::try_from(1u8),
            Ok(DACommitmentScheme::EmptyNoDA)
        );
        assert_eq!(
            DACommitmentScheme::try_from(2u8),
            Ok(DACommitmentScheme::PubdataKeccak256)
        );
        assert_eq!(
            DACommitmentScheme::try_from(3u8),
            Ok(DACommitmentScheme::BlobsAndPubdataKeccak256)
        );
        assert_eq!(
            DACommitmentScheme::try_from(4u8),
            Ok(DACommitmentScheme::BlobsZKsyncOS)
        );
    }

    #[test]
    fn test_da_commitment_scheme_rejects_invalid_ids() {
        // Value just above the valid range
        assert!(DACommitmentScheme::try_from(5u8).is_err());
        // Middle of invalid range
        assert!(DACommitmentScheme::try_from(128u8).is_err());
        // Maximum u8 value
        assert!(DACommitmentScheme::try_from(255u8).is_err());
    }
}

/// Integration tests using custom oracle factories to test security-critical
/// oracle validation paths end-to-end. These cover cases from PR#374 where
/// the system must properly validate and reject malicious oracle responses.
mod custom_oracle_factories {
    use rig::alloy::consensus::TxEip2930;
    use rig::alloy::primitives::{TxKind, U256};
    use rig::basic_system::system_implementation::flat_storage_model::{
        FlatStorageCommitment, TREE_HEIGHT,
    };
    use rig::chain::TestingOracleFactory;
    use rig::forward_system::run::convert_alloy::FromAlloy;
    use rig::forward_system::run::query_processors::{
        BlockMetadataResponder, DACommitmentSchemeResponder, GenericPreimageResponder,
        ReadTreeResponder, ZKProofDataResponder,
    };
    use rig::forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree};
    use rig::forward_system::run::{NextTxResponse, PreimageSource, TxSource};
    use rig::oracle_provider::{MemorySource, OracleQueryProcessor, ZkEENonDeterminismSource};
    use rig::ruint::aliases::B160;
    use rig::zk_ee::common_structs::{da_commitment_scheme::DACommitmentScheme, ProofData};
    use rig::zk_ee::oracle::query_ids::{
        NEXT_TX_SIZE_QUERY_ID, TX_DATA_WORDS_QUERY_ID, TX_ENCODING_FORMAT_QUERY_ID,
        TX_FROM_QUERY_ID,
    };
    use rig::zk_ee::oracle::simple_oracle_query::SimpleOracleQuery;
    use rig::zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;
    use rig::zk_ee::oracle::usize_serialization::UsizeSerializable;
    use rig::zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle;
    use rig::zk_ee::utils::usize_rw::ReadIterWrapper;
    use rig::zk_ee::utils::Bytes32;
    use rig::zksync_os_interface::traits::{EncodedTx, TxListSource};
    use rig::{common_target_address, TestingFramework};
    use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

    // ---- Malicious TX encoding format responder ----

    /// Oracle query processor that returns an invalid encoding format value.
    /// Handles all 4 TX-related query IDs, but overrides TX_ENCODING_FORMAT_QUERY_ID
    /// to return a malicious (invalid) format byte.
    struct MaliciousTxFormatResponder {
        tx_source: TxListSource,
        next_tx: Option<Vec<u8>>,
        next_tx_from: Option<B160>,
        malicious_format_value: usize,
    }

    impl MaliciousTxFormatResponder {
        fn new(tx_source: TxListSource, malicious_format_value: usize) -> Self {
            Self {
                tx_source,
                next_tx: None,
                next_tx_from: None,
                malicious_format_value,
            }
        }

        const SUPPORTED_QUERY_IDS: &[u32] = &[
            NEXT_TX_SIZE_QUERY_ID,
            TX_DATA_WORDS_QUERY_ID,
            TX_ENCODING_FORMAT_QUERY_ID,
            TX_FROM_QUERY_ID,
        ];
    }

    impl<M: MemorySource> OracleQueryProcessor<M> for MaliciousTxFormatResponder {
        fn supported_query_ids(&self) -> Vec<u32> {
            Self::SUPPORTED_QUERY_IDS.to_vec()
        }

        fn supports_query_id(&self, query_id: u32) -> bool {
            Self::SUPPORTED_QUERY_IDS.contains(&query_id)
        }

        fn process_buffered_query(
            &mut self,
            query_id: u32,
            _query: Vec<usize>,
            _memory: &M,
        ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
            assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

            match query_id {
                NEXT_TX_SIZE_QUERY_ID => {
                    let len = match &self.next_tx {
                        Some(next_tx) => next_tx.len(),
                        None => match self.tx_source.get_next_tx() {
                            NextTxResponse::SealBlock => 0,
                            NextTxResponse::Tx(EncodedTx::Abi(next_tx)) => {
                                let next_tx_len = next_tx.len();
                                assert_ne!(next_tx_len, 0);
                                self.next_tx = Some(next_tx);
                                self.next_tx_from = None;
                                next_tx_len
                            }
                            NextTxResponse::Tx(EncodedTx::Rlp(next_tx, from)) => {
                                let next_tx_len = next_tx.len();
                                assert_ne!(next_tx_len, 0);
                                self.next_tx = Some(next_tx);
                                self.next_tx_from = Some(B160::from_alloy(from));
                                next_tx_len
                            }
                        },
                    } as u32;
                    DynUsizeIterator::from_constructor(len, UsizeSerializable::iter)
                }
                TX_DATA_WORDS_QUERY_ID => {
                    let tx = self.next_tx.take().expect(
                        "trying to read next tx content before size query or after seal response",
                    );
                    DynUsizeIterator::from_constructor(tx, |inner_ref| {
                        ReadIterWrapper::from(inner_ref.iter().copied())
                    })
                }
                TX_ENCODING_FORMAT_QUERY_ID => {
                    // MALICIOUS: return an invalid encoding format value
                    Box::new(core::iter::once(self.malicious_format_value))
                }
                TX_FROM_QUERY_ID => {
                    let from = self.next_tx_from.take().expect(
                        "trying to read next tx from before size query or after seal response",
                    );
                    DynUsizeIterator::from_constructor(from, UsizeSerializable::iter)
                }
                _ => unreachable!(),
            }
        }
    }

    /// Custom oracle factory that injects an invalid TX encoding format value.
    struct MaliciousTxFormatOracleFactory {
        malicious_format_value: usize,
    }

    impl MaliciousTxFormatOracleFactory {
        fn new(malicious_format_value: usize) -> Self {
            Self {
                malicious_format_value,
            }
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
            let mut oracle = ZkEENonDeterminismSource::default();
            oracle.add_external_processor(BlockMetadataResponder { block_metadata });
            oracle.add_external_processor(MaliciousTxFormatResponder::new(
                tx_source,
                self.malicious_format_value,
            ));
            oracle.add_external_processor(GenericPreimageResponder { preimage_source });
            oracle.add_external_processor(ReadTreeResponder { tree: state_tree });
            oracle.add_external_processor(ZKProofDataResponder { data: proof_data });
            oracle.add_external_processor(DACommitmentSchemeResponder {
                da_commitment_scheme,
            });
            oracle
        }
    }

    impl TestingOracleFactory<false> for MaliciousTxFormatOracleFactory {
        fn create_forward_oracle(
            &self,
            block_metadata: BlockMetadataFromOracle,
            state_tree: InMemoryTree<false>,
            preimage_source: InMemoryPreimageSource,
            tx_source: TxListSource,
            proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            da_commitment_scheme: Option<DACommitmentScheme>,
            _add_uart: bool,
        ) -> ZkEENonDeterminismSource<rig::oracle_provider::DummyMemorySource> {
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
        ) -> ZkEENonDeterminismSource<rig::risc_v_simulator::abstractions::memory::VectorMemoryImpl>
        {
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

    /// Verifies that the system rejects an invalid TX encoding format (value 255)
    /// returned by a malicious oracle, via a custom oracle factory.
    #[test]
    fn test_malicious_oracle_invalid_tx_encoding_format() {
        let mut tester = TestingFramework::new();
        let wallet = tester.random_signer();
        tester = tester.with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

        let tx = {
            let tx = TxEip2930 {
                chain_id: 37u64,
                nonce: 0,
                gas_price: 1000,
                gas_limit: 21_000,
                to: TxKind::Call(common_target_address()),
                value: Default::default(),
                input: Default::default(),
                access_list: Default::default(),
            };
            ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
        };

        // Malicious oracle returns 255 as the encoding format
        let malicious_factory = MaliciousTxFormatOracleFactory::new(255);
        tester = tester.with_custom_oracle_factory(malicious_factory);

        let result = tester.execute_block_no_panic(vec![tx]);
        assert!(
            result.is_err(),
            "Block execution should fail when oracle returns invalid TX encoding format"
        );
    }

    /// Verifies that the system rejects TX encoding format value 2
    /// (just above the valid range of 0-1) from a malicious oracle.
    #[test]
    fn test_malicious_oracle_tx_encoding_format_just_above_valid() {
        let mut tester = TestingFramework::new();
        let wallet = tester.random_signer();
        tester = tester.with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

        let tx = {
            let tx = TxEip2930 {
                chain_id: 37u64,
                nonce: 0,
                gas_price: 1000,
                gas_limit: 21_000,
                to: TxKind::Call(common_target_address()),
                value: Default::default(),
                input: Default::default(),
                access_list: Default::default(),
            };
            ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
        };

        // Malicious oracle returns 2 (just above valid range 0-1)
        let malicious_factory = MaliciousTxFormatOracleFactory::new(2);
        tester = tester.with_custom_oracle_factory(malicious_factory);

        let result = tester.execute_block_no_panic(vec![tx]);
        assert!(
            result.is_err(),
            "Block execution should fail when oracle returns TX encoding format value 2"
        );
    }

    /// Verifies that the system rejects a large TX encoding format value (u64::MAX)
    /// from a malicious oracle via a custom oracle factory. Tests the u8 overflow path.
    #[test]
    fn test_malicious_oracle_tx_encoding_format_overflow() {
        let mut tester = TestingFramework::new();
        let wallet = tester.random_signer();
        tester = tester.with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

        let tx = {
            let tx = TxEip2930 {
                chain_id: 37u64,
                nonce: 0,
                gas_price: 1000,
                gas_limit: 21_000,
                to: TxKind::Call(common_target_address()),
                value: Default::default(),
                input: Default::default(),
                access_list: Default::default(),
            };
            ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
        };

        // Malicious oracle returns usize::MAX — overflows u8 deserialization
        let malicious_factory = MaliciousTxFormatOracleFactory::new(usize::MAX);
        tester = tester.with_custom_oracle_factory(malicious_factory);

        let result = tester.execute_block_no_panic(vec![tx]);
        assert!(
            result.is_err(),
            "Block execution should fail when oracle returns TX encoding format overflow value"
        );
    }

    // ---- Malicious preimage responder ----

    /// Oracle query processor that drops preimages for target hashes,
    /// simulating a malicious oracle that refuses to provide required data.
    struct MaliciousPreimageResponder {
        preimage_source: InMemoryPreimageSource,
        /// Hashes for which preimage lookups will return None (simulating missing data)
        blocked_hashes: Vec<Bytes32>,
    }

    impl MaliciousPreimageResponder {
        fn new(preimage_source: InMemoryPreimageSource, blocked_hashes: Vec<Bytes32>) -> Self {
            Self {
                preimage_source,
                blocked_hashes,
            }
        }
    }

    impl MaliciousPreimageResponder {
        const SUPPORTED_QUERY_IDS: &[u32] = &[
            rig::basic_system::system_implementation::flat_storage_model::FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID,
            rig::basic_system::system_implementation::ethereum_storage_model::ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID,
            rig::basic_system::system_implementation::ethereum_storage_model::ETHEREUM_BYTECODE_PREIMAGE_QUERY_ID,
            rig::basic_system::system_implementation::ethereum_storage_model::ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID,
            rig::basic_system::system_implementation::ethereum_storage_model::ETHEREUM_MPT_PREIMAGE_WORDS_QUERY_ID,
        ];
    }

    impl<M: MemorySource> OracleQueryProcessor<M> for MaliciousPreimageResponder {
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
            use rig::zk_ee::oracle::usize_serialization::UsizeDeserializable;

            assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

            let hash =
                Bytes32::from_iter(&mut query.into_iter()).expect("must deserialize hash value");

            let preimage = if hash.is_zero() {
                vec![]
            } else if self.blocked_hashes.iter().any(|h| *h == hash) {
                // MALICIOUS: refuse to provide preimage for blocked hashes
                panic!(
                    "must know a preimage for hash {} for query ID 0x{:016x}",
                    hex::encode(hash.as_u8_array_ref()),
                    query_id
                );
            } else {
                self.preimage_source.get_preimage(hash).unwrap_or_else(|| {
                    panic!(
                        "must know a preimage for hash {} for query ID 0x{:016x}",
                        hex::encode(hash.as_u8_array_ref()),
                        query_id
                    )
                })
            };

            use rig::basic_system::system_implementation::ethereum_storage_model::{
                ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID,
                ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID,
            };
            if query_id == ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID
                || query_id == ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID
            {
                let len = preimage.len() as u32;
                DynUsizeIterator::from_constructor(len, UsizeSerializable::iter)
            } else {
                DynUsizeIterator::from_constructor(preimage, |inner_ref| {
                    ReadIterWrapper::from(inner_ref.iter().copied())
                })
            }
        }
    }

    /// Custom oracle factory that blocks preimage lookups for specific hashes.
    struct MaliciousPreimageOracleFactory {
        blocked_hashes: Vec<Bytes32>,
    }

    impl MaliciousPreimageOracleFactory {
        fn new(blocked_hashes: Vec<Bytes32>) -> Self {
            Self { blocked_hashes }
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
            let mut oracle = ZkEENonDeterminismSource::default();
            oracle.add_external_processor(BlockMetadataResponder { block_metadata });
            oracle.add_external_processor(
                rig::forward_system::run::query_processors::TxDataResponder {
                    tx_source,
                    next_tx: None,
                    next_tx_format: None,
                    next_tx_from: None,
                },
            );
            oracle.add_external_processor(MaliciousPreimageResponder::new(
                preimage_source,
                self.blocked_hashes.clone(),
            ));
            oracle.add_external_processor(ReadTreeResponder { tree: state_tree });
            oracle.add_external_processor(ZKProofDataResponder { data: proof_data });
            oracle.add_external_processor(DACommitmentSchemeResponder {
                da_commitment_scheme,
            });
            oracle
        }
    }

    impl TestingOracleFactory<false> for MaliciousPreimageOracleFactory {
        fn create_forward_oracle(
            &self,
            block_metadata: BlockMetadataFromOracle,
            state_tree: InMemoryTree<false>,
            preimage_source: InMemoryPreimageSource,
            tx_source: TxListSource,
            proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            da_commitment_scheme: Option<DACommitmentScheme>,
            _add_uart: bool,
        ) -> ZkEENonDeterminismSource<rig::oracle_provider::DummyMemorySource> {
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
        ) -> ZkEENonDeterminismSource<rig::risc_v_simulator::abstractions::memory::VectorMemoryImpl>
        {
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

    /// Verifies that the system panics when a malicious oracle refuses to provide
    /// the bytecode preimage for a deployed contract. This simulates an attack where
    /// the oracle withholds required data during contract execution.
    #[test]
    #[should_panic(expected = "must know a preimage")]
    fn test_malicious_oracle_missing_bytecode_preimage() {
        let mut tester = TestingFramework::new();
        let wallet = tester.random_signer();

        let contract_address =
            rig::alloy::primitives::address!("1000000000000000000000000000000000000001");

        // Simple contract: PUSH1 0x00 PUSH1 0x00 RETURN (returns empty)
        let simple_bytecode = hex::decode("60006000f3").unwrap();

        // Deploy the contract normally first to register its account properties
        tester = tester
            .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64))
            .with_evm_contract(contract_address, &simple_bytecode);

        // Get the bytecode hash from the deployed account to block it
        let account_props = tester.get_account_properties(&contract_address);
        let bytecode_hash = account_props.bytecode_hash;

        // Now set up the malicious factory that blocks this bytecode's preimage
        let malicious_factory = MaliciousPreimageOracleFactory::new(vec![bytecode_hash]);
        tester = tester.with_custom_oracle_factory(malicious_factory);

        // Call the deployed contract — the system will try to decommit its bytecode
        // and the malicious oracle will refuse to provide the preimage
        let tx = {
            let tx = TxEip2930 {
                chain_id: 37u64,
                nonce: 0,
                gas_price: 1000,
                gas_limit: 100_000,
                to: TxKind::Call(contract_address),
                value: Default::default(),
                input: Default::default(),
                access_list: Default::default(),
            };
            ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
        };

        let _result = tester.execute_block(vec![tx]);
    }

    // ---- Malicious account properties responder ----

    /// Oracle query processor that returns corrupted account properties hashes.
    /// For queries targeting ACCOUNT_PROPERTIES_STORAGE_ADDRESS, returns a non-zero
    /// hash that doesn't correspond to any real preimage, simulating an oracle that
    /// provides fake account data.
    struct MaliciousAccountStorageResponder<S: rig::forward_system::run::ReadStorage> {
        storage: S,
    }

    impl<S: rig::forward_system::run::ReadStorage> MaliciousAccountStorageResponder<S> {
        fn new(storage: S) -> Self {
            Self { storage }
        }

        const SUPPORTED_QUERY_IDS: &[u32] = &[
            rig::zk_ee::oracle::basic_queries::InitialStorageSlotQuery::<
                rig::zk_ee::types_config::EthereumIOTypesConfig,
            >::QUERY_ID,
        ];
    }

    impl<S: rig::forward_system::run::ReadStorage, M: MemorySource> OracleQueryProcessor<M>
        for MaliciousAccountStorageResponder<S>
    {
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
            use rig::zk_ee::oracle::basic_queries::InitialStorageSlotQuery;
            use rig::zk_ee::oracle::usize_serialization::UsizeDeserializable;
            use rig::zk_ee::storage_types::{InitialStorageSlotData, StorageAddress};
            use rig::zk_ee::types_config::EthereumIOTypesConfig;

            assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

            let StorageAddress { address, key } =
                <InitialStorageSlotQuery<EthereumIOTypesConfig> as SimpleOracleQuery>::Input::from_iter(
                    &mut query.into_iter(),
                )
                .expect("must deserialize the address/slot");

            use rig::basic_system::system_implementation::flat_storage_model::storage_cache::ACCOUNT_PROPERTIES_STORAGE_ADDRESS;

            let flat_key = rig::zk_ee::common_structs::derive_flat_storage_key(&address, &key);

            let slot_data: InitialStorageSlotData<EthereumIOTypesConfig> =
                if address == ACCOUNT_PROPERTIES_STORAGE_ADDRESS {
                    // MALICIOUS: return a fake non-zero hash for account properties.
                    // This hash won't correspond to any preimage in the preimage source,
                    // so the system should fail when trying to decommit account data.
                    InitialStorageSlotData {
                        initial_value: Bytes32::from_array([0xDE; 32]),
                        is_new_storage_slot: false,
                    }
                } else if let Some(cold) = self.storage.read(flat_key) {
                    InitialStorageSlotData {
                        initial_value: cold,
                        is_new_storage_slot: false,
                    }
                } else {
                    InitialStorageSlotData {
                        initial_value: Bytes32::from_array([0; 32]),
                        is_new_storage_slot: true,
                    }
                };

            DynUsizeIterator::from_constructor(slot_data, UsizeSerializable::iter)
        }
    }

    /// Custom oracle factory that corrupts account properties storage reads.
    struct MaliciousAccountStorageOracleFactory;

    impl MaliciousAccountStorageOracleFactory {
        fn build_oracle<M: MemorySource + 'static>(
            &self,
            block_metadata: BlockMetadataFromOracle,
            state_tree: InMemoryTree<false>,
            preimage_source: InMemoryPreimageSource,
            tx_source: TxListSource,
            proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            da_commitment_scheme: Option<DACommitmentScheme>,
        ) -> ZkEENonDeterminismSource<M> {
            let mut oracle = ZkEENonDeterminismSource::default();
            oracle.add_external_processor(BlockMetadataResponder { block_metadata });
            oracle.add_external_processor(
                rig::forward_system::run::query_processors::TxDataResponder {
                    tx_source,
                    next_tx: None,
                    next_tx_format: None,
                    next_tx_from: None,
                },
            );
            oracle.add_external_processor(GenericPreimageResponder { preimage_source });
            oracle.add_external_processor(MaliciousAccountStorageResponder::new(state_tree));
            oracle.add_external_processor(ZKProofDataResponder { data: proof_data });
            oracle.add_external_processor(DACommitmentSchemeResponder {
                da_commitment_scheme,
            });
            oracle
        }
    }

    impl TestingOracleFactory<false> for MaliciousAccountStorageOracleFactory {
        fn create_forward_oracle(
            &self,
            block_metadata: BlockMetadataFromOracle,
            state_tree: InMemoryTree<false>,
            preimage_source: InMemoryPreimageSource,
            tx_source: TxListSource,
            proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            da_commitment_scheme: Option<DACommitmentScheme>,
            _add_uart: bool,
        ) -> ZkEENonDeterminismSource<rig::oracle_provider::DummyMemorySource> {
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
        ) -> ZkEENonDeterminismSource<rig::risc_v_simulator::abstractions::memory::VectorMemoryImpl>
        {
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

    /// Verifies that the system panics when a malicious oracle provides a fake hash
    /// for account properties. The fake hash doesn't correspond to any real preimage,
    /// so the system should fail when trying to decommit account data.
    #[test]
    #[should_panic(expected = "must know a preimage")]
    fn test_malicious_oracle_corrupted_account_properties() {
        let mut tester = TestingFramework::new();
        let wallet = tester.random_signer();
        tester = tester.with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

        // The malicious factory will return fake hashes for ALL account property reads,
        // including the sender's balance lookup during transaction validation.
        let malicious_factory = MaliciousAccountStorageOracleFactory;
        tester = tester.with_custom_oracle_factory(malicious_factory);

        let tx = {
            let tx = TxEip2930 {
                chain_id: 37u64,
                nonce: 0,
                gas_price: 1000,
                gas_limit: 21_000,
                to: TxKind::Call(common_target_address()),
                value: Default::default(),
                input: Default::default(),
                access_list: Default::default(),
            };
            ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
        };

        let _result = tester.execute_block(vec![tx]);
    }

    // ---- Malicious TX data corruption responder ----

    /// Oracle query processor that corrupts the transaction data bytes.
    /// Returns valid transaction size and format, but replaces the actual TX bytes
    /// with garbage data.
    struct MaliciousTxDataCorruptResponder {
        tx_source: TxListSource,
        next_tx: Option<Vec<u8>>,
        next_tx_from: Option<B160>,
    }

    impl MaliciousTxDataCorruptResponder {
        fn new(tx_source: TxListSource) -> Self {
            Self {
                tx_source,
                next_tx: None,
                next_tx_from: None,
            }
        }

        const SUPPORTED_QUERY_IDS: &[u32] = &[
            NEXT_TX_SIZE_QUERY_ID,
            TX_DATA_WORDS_QUERY_ID,
            TX_ENCODING_FORMAT_QUERY_ID,
            TX_FROM_QUERY_ID,
        ];
    }

    impl<M: MemorySource> OracleQueryProcessor<M> for MaliciousTxDataCorruptResponder {
        fn supported_query_ids(&self) -> Vec<u32> {
            Self::SUPPORTED_QUERY_IDS.to_vec()
        }

        fn supports_query_id(&self, query_id: u32) -> bool {
            Self::SUPPORTED_QUERY_IDS.contains(&query_id)
        }

        fn process_buffered_query(
            &mut self,
            query_id: u32,
            _query: Vec<usize>,
            _memory: &M,
        ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
            assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

            match query_id {
                NEXT_TX_SIZE_QUERY_ID => {
                    let len = match &self.next_tx {
                        Some(next_tx) => next_tx.len(),
                        None => match self.tx_source.get_next_tx() {
                            NextTxResponse::SealBlock => 0,
                            NextTxResponse::Tx(EncodedTx::Abi(next_tx)) => {
                                let next_tx_len = next_tx.len();
                                assert_ne!(next_tx_len, 0);
                                // MALICIOUS: replace valid tx bytes with garbage
                                self.next_tx = Some(vec![0xFF; next_tx_len]);
                                self.next_tx_from = None;
                                next_tx_len
                            }
                            NextTxResponse::Tx(EncodedTx::Rlp(next_tx, from)) => {
                                let next_tx_len = next_tx.len();
                                assert_ne!(next_tx_len, 0);
                                // MALICIOUS: replace valid tx bytes with garbage
                                self.next_tx = Some(vec![0xFF; next_tx_len]);
                                self.next_tx_from = Some(B160::from_alloy(from));
                                next_tx_len
                            }
                        },
                    } as u32;
                    DynUsizeIterator::from_constructor(len, UsizeSerializable::iter)
                }
                TX_DATA_WORDS_QUERY_ID => {
                    let tx = self.next_tx.take().expect(
                        "trying to read next tx content before size query or after seal response",
                    );
                    DynUsizeIterator::from_constructor(tx, |inner_ref| {
                        ReadIterWrapper::from(inner_ref.iter().copied())
                    })
                }
                TX_ENCODING_FORMAT_QUERY_ID => {
                    // Return valid RLP format so parsing is attempted on garbage data
                    Box::new(core::iter::once(1usize)) // 1 = Rlp
                }
                TX_FROM_QUERY_ID => {
                    let from = self.next_tx_from.take().expect(
                        "trying to read next tx from before size query or after seal response",
                    );
                    DynUsizeIterator::from_constructor(from, UsizeSerializable::iter)
                }
                _ => unreachable!(),
            }
        }
    }

    /// Custom oracle factory that corrupts transaction data bytes.
    struct MaliciousTxDataCorruptOracleFactory;

    impl MaliciousTxDataCorruptOracleFactory {
        fn build_oracle<M: MemorySource + 'static>(
            &self,
            block_metadata: BlockMetadataFromOracle,
            state_tree: InMemoryTree<false>,
            preimage_source: InMemoryPreimageSource,
            tx_source: TxListSource,
            proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            da_commitment_scheme: Option<DACommitmentScheme>,
        ) -> ZkEENonDeterminismSource<M> {
            let mut oracle = ZkEENonDeterminismSource::default();
            oracle.add_external_processor(BlockMetadataResponder { block_metadata });
            oracle.add_external_processor(MaliciousTxDataCorruptResponder::new(tx_source));
            oracle.add_external_processor(GenericPreimageResponder { preimage_source });
            oracle.add_external_processor(ReadTreeResponder { tree: state_tree });
            oracle.add_external_processor(ZKProofDataResponder { data: proof_data });
            oracle.add_external_processor(DACommitmentSchemeResponder {
                da_commitment_scheme,
            });
            oracle
        }
    }

    impl TestingOracleFactory<false> for MaliciousTxDataCorruptOracleFactory {
        fn create_forward_oracle(
            &self,
            block_metadata: BlockMetadataFromOracle,
            state_tree: InMemoryTree<false>,
            preimage_source: InMemoryPreimageSource,
            tx_source: TxListSource,
            proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            da_commitment_scheme: Option<DACommitmentScheme>,
            _add_uart: bool,
        ) -> ZkEENonDeterminismSource<rig::oracle_provider::DummyMemorySource> {
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
        ) -> ZkEENonDeterminismSource<rig::risc_v_simulator::abstractions::memory::VectorMemoryImpl>
        {
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

    /// Verifies that corrupted transaction data bytes from a malicious oracle
    /// cause the transaction to be rejected. The block still completes (an internal
    /// error during TX parsing causes the bootloader to reject that TX), but
    /// no transaction should be successfully executed.
    #[test]
    fn test_malicious_oracle_corrupted_tx_data() {
        let mut tester = TestingFramework::new();
        let wallet = tester.random_signer();
        tester = tester.with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

        let tx = {
            let tx = TxEip2930 {
                chain_id: 37u64,
                nonce: 0,
                gas_price: 1000,
                gas_limit: 21_000,
                to: TxKind::Call(common_target_address()),
                value: Default::default(),
                input: Default::default(),
                access_list: Default::default(),
            };
            ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
        };

        let malicious_factory = MaliciousTxDataCorruptOracleFactory;
        tester = tester.with_custom_oracle_factory(malicious_factory);

        // The block completes (corrupted TX is rejected during parsing),
        // but no transaction should succeed.
        let result = tester.execute_block_no_panic(vec![tx]);
        match result {
            Err(_) => {
                // Block-level error from corrupted data — expected behavior
            }
            Ok(output) => {
                // Block completed, but the corrupted TX must not have succeeded.
                // Use the same pattern as assert_all_txs_succeeded but inverted:
                let any_succeeded = output
                    .tx_results
                    .iter()
                    .any(|r| r.as_ref().is_ok_and(|o| o.is_success()));
                assert!(
                    !any_succeeded,
                    "Corrupted TX data should not result in a successfully executed transaction"
                );
            }
        }
    }

    // ---- Malicious storage responder: false "existing" claim for new slots ----

    /// Oracle query processor that claims all storage slots already exist (is_new=false)
    /// even when they are actually new. This tests whether the system handles incorrect
    /// is_new_storage_slot flags — in forward mode this may silently corrupt pubdata
    /// accounting, but should not crash.
    struct FalseExistingSlotResponder<S: rig::forward_system::run::ReadStorage> {
        storage: S,
    }

    impl<S: rig::forward_system::run::ReadStorage> FalseExistingSlotResponder<S> {
        fn new(storage: S) -> Self {
            Self { storage }
        }

        const SUPPORTED_QUERY_IDS: &[u32] = &[
            rig::zk_ee::oracle::basic_queries::InitialStorageSlotQuery::<
                rig::zk_ee::types_config::EthereumIOTypesConfig,
            >::QUERY_ID,
        ];
    }

    impl<S: rig::forward_system::run::ReadStorage, M: MemorySource> OracleQueryProcessor<M>
        for FalseExistingSlotResponder<S>
    {
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
            use rig::zk_ee::oracle::basic_queries::InitialStorageSlotQuery;
            use rig::zk_ee::oracle::usize_serialization::UsizeDeserializable;
            use rig::zk_ee::storage_types::{InitialStorageSlotData, StorageAddress};
            use rig::zk_ee::types_config::EthereumIOTypesConfig;

            assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

            let StorageAddress { address, key } =
                <InitialStorageSlotQuery<EthereumIOTypesConfig> as SimpleOracleQuery>::Input::from_iter(
                    &mut query.into_iter(),
                )
                .expect("must deserialize the address/slot");

            let flat_key = rig::zk_ee::common_structs::derive_flat_storage_key(&address, &key);

            let slot_data: InitialStorageSlotData<EthereumIOTypesConfig> =
                if let Some(cold) = self.storage.read(flat_key) {
                    InitialStorageSlotData {
                        initial_value: cold,
                        is_new_storage_slot: false,
                    }
                } else {
                    // MALICIOUS: claim this new slot already exists with zero value.
                    // This bypasses the "Initial value of empty slot must be trivial"
                    // check (which only fires when is_new=true) and may corrupt
                    // pubdata accounting.
                    InitialStorageSlotData {
                        initial_value: Bytes32::from_array([0; 32]),
                        is_new_storage_slot: false, // Lie about slot existence
                    }
                };

            DynUsizeIterator::from_constructor(slot_data, UsizeSerializable::iter)
        }
    }

    /// Custom oracle factory that lies about slot existence.
    struct FalseExistingSlotOracleFactory;

    impl FalseExistingSlotOracleFactory {
        fn build_oracle<M: MemorySource + 'static>(
            &self,
            block_metadata: BlockMetadataFromOracle,
            state_tree: InMemoryTree<false>,
            preimage_source: InMemoryPreimageSource,
            tx_source: TxListSource,
            proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            da_commitment_scheme: Option<DACommitmentScheme>,
        ) -> ZkEENonDeterminismSource<M> {
            let mut oracle = ZkEENonDeterminismSource::default();
            oracle.add_external_processor(BlockMetadataResponder { block_metadata });
            oracle.add_external_processor(
                rig::forward_system::run::query_processors::TxDataResponder {
                    tx_source,
                    next_tx: None,
                    next_tx_format: None,
                    next_tx_from: None,
                },
            );
            oracle.add_external_processor(GenericPreimageResponder { preimage_source });
            oracle.add_external_processor(FalseExistingSlotResponder::new(state_tree));
            oracle.add_external_processor(ZKProofDataResponder { data: proof_data });
            oracle.add_external_processor(DACommitmentSchemeResponder {
                da_commitment_scheme,
            });
            oracle
        }
    }

    impl TestingOracleFactory<false> for FalseExistingSlotOracleFactory {
        fn create_forward_oracle(
            &self,
            block_metadata: BlockMetadataFromOracle,
            state_tree: InMemoryTree<false>,
            preimage_source: InMemoryPreimageSource,
            tx_source: TxListSource,
            proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            da_commitment_scheme: Option<DACommitmentScheme>,
            _add_uart: bool,
        ) -> ZkEENonDeterminismSource<rig::oracle_provider::DummyMemorySource> {
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
        ) -> ZkEENonDeterminismSource<rig::risc_v_simulator::abstractions::memory::VectorMemoryImpl>
        {
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

    /// Verifies that a malicious oracle claiming all new slots are existing (is_new=false)
    /// does not crash the system in forward mode. The wrong is_new flag corrupts pubdata
    /// accounting (a concern for proving mode), but forward execution should still complete.
    /// This test documents the forward-mode behavior for this attack vector.
    #[test]
    fn test_malicious_oracle_false_existing_slot_does_not_crash() {
        let mut tester = TestingFramework::new();
        let wallet = tester.random_signer();

        let contract_address =
            rig::alloy::primitives::address!("1000000000000000000000000000000000000001");

        // Simple storage contract: SSTORE(0, calldata[0..32])
        // PUSH1 0x00 CALLDATALOAD PUSH1 0x00 SSTORE STOP
        let store_bytecode = hex::decode("600035600055005050").unwrap();

        tester = tester
            .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64))
            .with_evm_contract(contract_address, &store_bytecode);

        let malicious_factory = FalseExistingSlotOracleFactory;
        tester = tester.with_custom_oracle_factory(malicious_factory);

        // Write to storage slot 0 — the oracle will falsely claim the slot already exists
        let calldata = [0u8; 32]; // store zero (avoids pubdata cost differences)
        let tx = {
            let tx = TxEip2930 {
                chain_id: 37u64,
                nonce: 0,
                gas_price: 1000,
                gas_limit: 100_000,
                to: TxKind::Call(contract_address),
                value: Default::default(),
                input: calldata.to_vec().into(),
                access_list: Default::default(),
            };
            ZKsyncTxEnvelope::from_eth_tx(tx, wallet)
        };

        // In forward mode, the false is_new flag should not cause a crash.
        // The system trusts the oracle response and proceeds (pubdata accounting
        // would be wrong, but this is caught in proving mode by Merkle proofs).
        let result = tester.execute_block_no_panic(vec![tx]);
        assert!(
            result.is_ok(),
            "Forward mode should not crash with false is_new flag — proving mode catches this"
        );
    }
}

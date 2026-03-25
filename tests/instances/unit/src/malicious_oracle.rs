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
    use rig::forward_system::run::{NextTxResponse, TxSource};
    use rig::oracle_provider::{MemorySource, OracleQueryProcessor, ZkEENonDeterminismSource};
    use rig::ruint::aliases::B160;
    use rig::zk_ee::common_structs::{da_commitment_scheme::DACommitmentScheme, ProofData};
    use rig::zk_ee::oracle::query_ids::{
        NEXT_TX_SIZE_QUERY_ID, TX_DATA_WORDS_QUERY_ID, TX_ENCODING_FORMAT_QUERY_ID,
        TX_FROM_QUERY_ID,
    };
    use rig::zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;
    use rig::zk_ee::oracle::usize_serialization::UsizeSerializable;
    use rig::zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle;
    use rig::zk_ee::utils::usize_rw::ReadIterWrapper;
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
}

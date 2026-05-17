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
        let err = result
            .expect_err("Block execution should fail when gas limit exceeds MAX_BLOCK_GAS_LIMIT");
        let err_message = err.to_string();
        assert!(
            err_message.contains("gas limit is too high"),
            "Expected gas limit error, got: {err_message}"
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
    /// This is a large overflow case -- u64::MAX is well above MAX_BLOCK_GAS_LIMIT.
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
        let err = result.expect_err("Even an empty block should fail with excessive gas limit");
        let err_message = err.to_string();
        assert!(
            err_message.contains("gas limit is too high"),
            "Expected gas limit error, got: {err_message}"
        );
    }
}

mod tx_encoding_format {
    //! Unit tests for transaction encoding format serde validation.
    //!
    //! TxEncodingFormat is deserialized from oracle data via serde.
    //! Only values 0 (Abi) and 1 (Rlp) are valid. Invalid values should be rejected
    //! with a deserialization error rather than panicking.

    use rig::basic_bootloader::bootloader::transaction::TxEncodingFormat;
    use rig::oracle_provider::airbender_codec::{AirbenderCodec, AirbenderCodecV0};

    #[test]
    fn test_tx_encoding_format_accepts_abi() {
        let encoded = AirbenderCodecV0::encode(&TxEncodingFormat::Abi).unwrap();
        let result: Result<TxEncodingFormat, _> = AirbenderCodecV0::decode(&encoded);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tx_encoding_format_accepts_rlp() {
        let encoded = AirbenderCodecV0::encode(&TxEncodingFormat::Rlp).unwrap();
        let result: Result<TxEncodingFormat, _> = AirbenderCodecV0::decode(&encoded);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tx_encoding_format_rejects_invalid_value_2() {
        // Encode value 2u8 and try to decode as TxEncodingFormat
        let encoded = AirbenderCodecV0::encode(&2u8).unwrap();
        let result: Result<TxEncodingFormat, _> = AirbenderCodecV0::decode(&encoded);
        assert!(
            result.is_err(),
            "TxEncodingFormat should reject value 2 (only 0=Abi and 1=Rlp are valid)"
        );
    }

    #[test]
    fn test_tx_encoding_format_rejects_invalid_value_255() {
        let encoded = AirbenderCodecV0::encode(&255u8).unwrap();
        let result: Result<TxEncodingFormat, _> = AirbenderCodecV0::decode(&encoded);
        assert!(result.is_err(), "TxEncodingFormat should reject value 255");
    }

    #[test]
    fn test_tx_encoding_format_rejects_large_value() {
        // Large u32 value that doesn't fit valid enum variants
        let encoded = AirbenderCodecV0::encode(&256u32).unwrap();
        let result: Result<TxEncodingFormat, _> = AirbenderCodecV0::decode(&encoded);
        assert!(result.is_err(), "TxEncodingFormat should reject value 256");
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
/// oracle validation paths end-to-end.
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
        ReadTreeResponder, TxDataResponder, ZKProofDataResponder,
    };
    use rig::forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree};
    use rig::forward_system::run::{NextTxResponse, PreimageSource};
    use rig::oracle_provider::airbender_codec::{AirbenderCodec, AirbenderCodecV0};
    use rig::oracle_provider::{OracleQueryProcessor, RamPeek, ZkEENonDeterminismSource};
    use rig::ruint::aliases::B160;
    use rig::zk_ee::common_structs::{
        da_commitment_scheme::DACommitmentScheme, derive_flat_storage_key, ProofData,
    };
    use rig::zk_ee::internal_error;
    use rig::zk_ee::oracle::basic_queries::InitialStorageSlotQuery;
    use rig::zk_ee::oracle::query_ids::{
        NEXT_TX_SIZE_QUERY_ID, TX_DATA_WORDS_QUERY_ID, TX_ENCODING_FORMAT_QUERY_ID,
        TX_FROM_QUERY_ID,
    };
    use rig::zk_ee::oracle::simple_oracle_query::SimpleOracleQuery;
    use rig::zk_ee::storage_types::{InitialStorageSlotData, StorageAddress};
    use rig::zk_ee::system::errors::internal::InternalError;
    use rig::zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle;
    use rig::zk_ee::types_config::EthereumIOTypesConfig;
    use rig::zk_ee::utils::Bytes32;
    use rig::zksync_os_interface::traits::{EncodedTx, TxListSource, TxSource};
    use rig::{common_target_address, TestingFramework};
    use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

    /// Generic oracle factory that delegates oracle construction to a closure.
    pub(super) struct CustomOracleFactory<F>(pub F)
    where
        F: Fn(
            BlockMetadataFromOracle,
            InMemoryTree<false>,
            InMemoryPreimageSource,
            TxListSource,
            Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            Option<DACommitmentScheme>,
        ) -> ZkEENonDeterminismSource;

    impl<F> TestingOracleFactory<false> for CustomOracleFactory<F>
    where
        F: Fn(
            BlockMetadataFromOracle,
            InMemoryTree<false>,
            InMemoryPreimageSource,
            TxListSource,
            Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            Option<DACommitmentScheme>,
        ) -> ZkEENonDeterminismSource,
    {
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
        ) -> ZkEENonDeterminismSource {
            (self.0)(
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
        ) -> ZkEENonDeterminismSource {
            (self.0)(
                block_metadata,
                state_tree,
                preimage_source,
                tx_source,
                proof_data,
                da_commitment_scheme,
            )
        }
    }

    /// Builds a standard oracle with common processors (block metadata, proof data,
    /// DA commitment scheme, field ops), calling `add_custom` to inject the
    /// test-specific (possibly malicious) processors.
    pub(super) fn build_oracle(
        block_metadata: BlockMetadataFromOracle,
        proof_data: Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        add_custom: impl FnOnce(&mut ZkEENonDeterminismSource),
    ) -> ZkEENonDeterminismSource {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(BlockMetadataResponder { block_metadata });
        add_custom(&mut oracle);
        oracle.add_external_processor(ZKProofDataResponder { data: proof_data });
        oracle.add_external_processor(DACommitmentSchemeResponder {
            da_commitment_scheme,
        });
        oracle.add_external_processor(rig::callable_oracles::field_hints::NativeFieldOpsQuery);
        oracle
    }

    /// Adds the standard (non-malicious) TX, preimage, and storage processors.
    pub(super) fn add_standard_processors(
        oracle: &mut ZkEENonDeterminismSource,
        state_tree: InMemoryTree<false>,
        preimage_source: InMemoryPreimageSource,
        tx_source: TxListSource,
    ) {
        oracle.add_external_processor(TxDataResponder {
            tx_source,
            next_tx: None,
            next_tx_format: None,
            next_tx_from: None,
        });
        oracle.add_external_processor(GenericPreimageResponder { preimage_source });
        oracle.add_external_processor(ReadTreeResponder { tree: state_tree });
    }

    // ---- Malicious TX encoding format responder ----

    /// Oracle query processor that returns an invalid encoding format value.
    /// Handles all 4 TX-related query IDs, but overrides TX_ENCODING_FORMAT_QUERY_ID
    /// to return a malicious (invalid) format byte.
    struct MaliciousTxFormatResponder {
        tx_source: TxListSource,
        next_tx: Option<Vec<u8>>,
        next_tx_from: Option<B160>,
        malicious_format_value: u8,
    }

    impl MaliciousTxFormatResponder {
        fn new(tx_source: TxListSource, malicious_format_value: u8) -> Self {
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

    impl OracleQueryProcessor for MaliciousTxFormatResponder {
        fn supported_query_ids(&self) -> Vec<u32> {
            Self::SUPPORTED_QUERY_IDS.to_vec()
        }

        fn supports_query_id(&self, query_id: u32) -> bool {
            Self::SUPPORTED_QUERY_IDS.contains(&query_id)
        }

        fn process(
            &mut self,
            query_id: u32,
            _input: &[u8],
            _memory: &dyn RamPeek,
        ) -> Result<Vec<u8>, InternalError> {
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
                    AirbenderCodecV0::encode(&len)
                        .map_err(|_| internal_error!("encode tx size failed"))
                }
                TX_DATA_WORDS_QUERY_ID => {
                    let tx = self.next_tx.take().expect(
                        "trying to read next tx content before size query or after seal response",
                    );
                    AirbenderCodecV0::encode(&tx)
                        .map_err(|_| internal_error!("encode tx data failed"))
                }
                TX_ENCODING_FORMAT_QUERY_ID => {
                    // MALICIOUS: return an invalid encoding format value
                    AirbenderCodecV0::encode(&self.malicious_format_value)
                        .map_err(|_| internal_error!("encode malicious format failed"))
                }
                TX_FROM_QUERY_ID => {
                    let from = self.next_tx_from.take().expect(
                        "trying to read next tx from before size query or after seal response",
                    );
                    AirbenderCodecV0::encode(&from)
                        .map_err(|_| internal_error!("encode tx from failed"))
                }
                _ => unreachable!(),
            }
        }
    }

    /// Helper: builds a CustomOracleFactory that injects a MaliciousTxFormatResponder.
    fn malicious_tx_format_factory(
        malicious_format_value: u8,
    ) -> CustomOracleFactory<
        impl Fn(
            BlockMetadataFromOracle,
            InMemoryTree<false>,
            InMemoryPreimageSource,
            TxListSource,
            Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            Option<DACommitmentScheme>,
        ) -> ZkEENonDeterminismSource,
    > {
        CustomOracleFactory(
            move |block_metadata,
                  state_tree,
                  preimage_source,
                  tx_source,
                  proof_data,
                  da_commitment_scheme| {
                build_oracle(block_metadata, proof_data, da_commitment_scheme, |oracle| {
                    oracle.add_external_processor(MaliciousTxFormatResponder::new(
                        tx_source,
                        malicious_format_value,
                    ));
                    oracle.add_external_processor(GenericPreimageResponder { preimage_source });
                    oracle.add_external_processor(ReadTreeResponder { tree: state_tree });
                })
            },
        )
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
        tester = tester.with_custom_oracle_factory(malicious_tx_format_factory(255));

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
        tester = tester.with_custom_oracle_factory(malicious_tx_format_factory(2));

        let result = tester.execute_block_no_panic(vec![tx]);
        assert!(
            result.is_err(),
            "Block execution should fail when oracle returns TX encoding format value 2"
        );
    }

    /// Verifies that the system rejects a large TX encoding format value (255)
    /// from a malicious oracle via a custom oracle factory.
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

        // Malicious oracle returns 255 -- an invalid enum discriminant
        tester = tester.with_custom_oracle_factory(malicious_tx_format_factory(255));

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

    impl OracleQueryProcessor for MaliciousPreimageResponder {
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
            _memory: &dyn RamPeek,
        ) -> Result<Vec<u8>, InternalError> {
            assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

            let hash: Bytes32 = AirbenderCodecV0::decode(input)
                .map_err(|_| internal_error!("decode hash failed"))?;

            let preimage = if hash.is_zero() {
                vec![]
            } else if self.blocked_hashes.iter().any(|h| *h == hash) {
                // MALICIOUS: refuse to provide preimage for blocked hashes.
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
                AirbenderCodecV0::encode(&len)
                    .map_err(|_| internal_error!("encode preimage length failed"))
            } else {
                AirbenderCodecV0::encode(&preimage)
                    .map_err(|_| internal_error!("encode preimage failed"))
            }
        }
    }

    /// Helper: builds a CustomOracleFactory that blocks preimage lookups for specific hashes.
    fn malicious_preimage_factory(
        blocked_hashes: Vec<Bytes32>,
    ) -> CustomOracleFactory<
        impl Fn(
            BlockMetadataFromOracle,
            InMemoryTree<false>,
            InMemoryPreimageSource,
            TxListSource,
            Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            Option<DACommitmentScheme>,
        ) -> ZkEENonDeterminismSource,
    > {
        CustomOracleFactory(
            move |block_metadata,
                  state_tree,
                  preimage_source,
                  tx_source,
                  proof_data,
                  da_commitment_scheme| {
                build_oracle(block_metadata, proof_data, da_commitment_scheme, |oracle| {
                    oracle.add_external_processor(TxDataResponder {
                        tx_source,
                        next_tx: None,
                        next_tx_format: None,
                        next_tx_from: None,
                    });
                    oracle.add_external_processor(MaliciousPreimageResponder::new(
                        preimage_source,
                        blocked_hashes.clone(),
                    ));
                    oracle.add_external_processor(ReadTreeResponder { tree: state_tree });
                })
            },
        )
    }

    /// Verifies that the system panics when a malicious oracle refuses to provide
    /// the bytecode preimage for a deployed contract.
    #[test]
    #[should_panic(expected = "must know a preimage")]
    fn test_malicious_oracle_missing_bytecode_preimage() {
        let mut tester = TestingFramework::new();
        let wallet = tester.random_signer();

        let contract_address =
            rig::alloy::primitives::address!("1000000000000000000000000000000000000001");

        // Simple contract: PUSH1 0x00 PUSH1 0x00 RETURN (returns empty)
        let simple_bytecode = hex::decode("60006000f3").unwrap();

        tester = tester
            .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64))
            .with_evm_contract(contract_address, &simple_bytecode);

        let account_props = tester.get_account_properties(&contract_address);
        let bytecode_hash = account_props.bytecode_hash;

        tester = tester.with_custom_oracle_factory(malicious_preimage_factory(vec![bytecode_hash]));

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
    struct MaliciousAccountStorageResponder<S: rig::forward_system::run::ReadStorage> {
        storage: S,
    }

    impl<S: rig::forward_system::run::ReadStorage> MaliciousAccountStorageResponder<S> {
        fn new(storage: S) -> Self {
            Self { storage }
        }

        const SUPPORTED_QUERY_IDS: &[u32] =
            &[InitialStorageSlotQuery::<EthereumIOTypesConfig>::QUERY_ID];
    }

    impl<S: rig::forward_system::run::ReadStorage> OracleQueryProcessor
        for MaliciousAccountStorageResponder<S>
    {
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
            _memory: &dyn RamPeek,
        ) -> Result<Vec<u8>, InternalError> {
            assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

            let StorageAddress { address, key }: StorageAddress<EthereumIOTypesConfig> =
                AirbenderCodecV0::decode(input)
                    .map_err(|_| internal_error!("decode StorageAddress failed"))?;

            use rig::basic_system::system_implementation::flat_storage_model::storage_cache::ACCOUNT_PROPERTIES_STORAGE_ADDRESS;

            let flat_key = derive_flat_storage_key(&address, &key);

            let slot_data: InitialStorageSlotData<EthereumIOTypesConfig> =
                if address == ACCOUNT_PROPERTIES_STORAGE_ADDRESS {
                    // MALICIOUS: return a fake non-zero hash for account properties.
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

            AirbenderCodecV0::encode(&slot_data)
                .map_err(|_| internal_error!("encode slot_data failed"))
        }
    }

    /// Helper: builds a CustomOracleFactory with MaliciousAccountStorageResponder.
    fn malicious_account_storage_factory() -> CustomOracleFactory<
        impl Fn(
            BlockMetadataFromOracle,
            InMemoryTree<false>,
            InMemoryPreimageSource,
            TxListSource,
            Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            Option<DACommitmentScheme>,
        ) -> ZkEENonDeterminismSource,
    > {
        CustomOracleFactory(
            |block_metadata,
             state_tree,
             preimage_source,
             tx_source,
             proof_data,
             da_commitment_scheme| {
                build_oracle(block_metadata, proof_data, da_commitment_scheme, |oracle| {
                    oracle.add_external_processor(TxDataResponder {
                        tx_source,
                        next_tx: None,
                        next_tx_format: None,
                        next_tx_from: None,
                    });
                    oracle.add_external_processor(GenericPreimageResponder { preimage_source });
                    oracle
                        .add_external_processor(MaliciousAccountStorageResponder::new(state_tree));
                })
            },
        )
    }

    /// Verifies that the system panics when a malicious oracle provides a fake hash
    /// for account properties.
    #[test]
    #[should_panic(expected = "must know a preimage")]
    fn test_malicious_oracle_corrupted_account_properties() {
        let mut tester = TestingFramework::new();
        let wallet = tester.random_signer();
        tester = tester.with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

        tester = tester.with_custom_oracle_factory(malicious_account_storage_factory());

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

    impl OracleQueryProcessor for MaliciousTxDataCorruptResponder {
        fn supported_query_ids(&self) -> Vec<u32> {
            Self::SUPPORTED_QUERY_IDS.to_vec()
        }

        fn supports_query_id(&self, query_id: u32) -> bool {
            Self::SUPPORTED_QUERY_IDS.contains(&query_id)
        }

        fn process(
            &mut self,
            query_id: u32,
            _input: &[u8],
            _memory: &dyn RamPeek,
        ) -> Result<Vec<u8>, InternalError> {
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
                    AirbenderCodecV0::encode(&len)
                        .map_err(|_| internal_error!("encode tx size failed"))
                }
                TX_DATA_WORDS_QUERY_ID => {
                    let tx = self.next_tx.take().expect(
                        "trying to read next tx content before size query or after seal response",
                    );
                    AirbenderCodecV0::encode(&tx)
                        .map_err(|_| internal_error!("encode tx data failed"))
                }
                TX_ENCODING_FORMAT_QUERY_ID => {
                    // Return valid RLP format so parsing is attempted on garbage data
                    use rig::basic_bootloader::bootloader::transaction::TxEncodingFormat;
                    AirbenderCodecV0::encode(&TxEncodingFormat::Rlp)
                        .map_err(|_| internal_error!("encode tx format failed"))
                }
                TX_FROM_QUERY_ID => {
                    let from = self.next_tx_from.take().expect(
                        "trying to read next tx from before size query or after seal response",
                    );
                    AirbenderCodecV0::encode(&from)
                        .map_err(|_| internal_error!("encode tx from failed"))
                }
                _ => unreachable!(),
            }
        }
    }

    /// Helper: builds a CustomOracleFactory with MaliciousTxDataCorruptResponder.
    fn malicious_tx_data_corrupt_factory() -> CustomOracleFactory<
        impl Fn(
            BlockMetadataFromOracle,
            InMemoryTree<false>,
            InMemoryPreimageSource,
            TxListSource,
            Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            Option<DACommitmentScheme>,
        ) -> ZkEENonDeterminismSource,
    > {
        CustomOracleFactory(
            |block_metadata,
             state_tree,
             preimage_source,
             tx_source,
             proof_data,
             da_commitment_scheme| {
                build_oracle(block_metadata, proof_data, da_commitment_scheme, |oracle| {
                    oracle.add_external_processor(MaliciousTxDataCorruptResponder::new(tx_source));
                    oracle.add_external_processor(GenericPreimageResponder { preimage_source });
                    oracle.add_external_processor(ReadTreeResponder { tree: state_tree });
                })
            },
        )
    }

    /// Verifies that corrupted transaction data bytes from a malicious oracle
    /// cause the transaction to be rejected.
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

        tester = tester.with_custom_oracle_factory(malicious_tx_data_corrupt_factory());

        let result = tester.execute_block_no_panic(vec![tx]);
        match result {
            Err(_) => {
                // Block-level error from corrupted data -- expected behavior
            }
            Ok(output) => {
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
    /// even when they are actually new.
    struct FalseExistingSlotResponder<S: rig::forward_system::run::ReadStorageTree> {
        storage: S,
    }

    impl<S: rig::forward_system::run::ReadStorageTree> FalseExistingSlotResponder<S> {
        fn new(storage: S) -> Self {
            Self { storage }
        }

        const SUPPORTED_QUERY_IDS: &[u32] = &[
            InitialStorageSlotQuery::<EthereumIOTypesConfig>::QUERY_ID,
            rig::basic_system::system_implementation::flat_storage_model::PreviousIndexQuery::QUERY_ID,
            rig::basic_system::system_implementation::flat_storage_model::ExactIndexQuery::QUERY_ID,
            rig::basic_system::system_implementation::flat_storage_model::PROOF_FOR_INDEX_QUERY_ID,
        ];
    }

    impl<S: rig::forward_system::run::ReadStorageTree> OracleQueryProcessor
        for FalseExistingSlotResponder<S>
    {
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
            _memory: &dyn RamPeek,
        ) -> Result<Vec<u8>, InternalError> {
            use rig::basic_system::system_implementation::flat_storage_model::{
                ExactIndexQuery, ExistingReadProof, PreviousIndexQuery, ValueAtIndexProof,
                PROOF_FOR_INDEX_QUERY_ID,
            };

            assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

            match query_id {
                _ if query_id == InitialStorageSlotQuery::<EthereumIOTypesConfig>::QUERY_ID => {
                    let StorageAddress { address, key }: StorageAddress<EthereumIOTypesConfig> =
                        AirbenderCodecV0::decode(input)
                            .map_err(|_| internal_error!("decode StorageAddress failed"))?;

                    let flat_key = derive_flat_storage_key(&address, &key);

                    let slot_data: InitialStorageSlotData<EthereumIOTypesConfig> =
                        if let Some(cold) = self.storage.read(flat_key) {
                            InitialStorageSlotData {
                                initial_value: cold,
                                is_new_storage_slot: false,
                            }
                        } else {
                            // MALICIOUS: claim this new slot already exists with zero value.
                            InitialStorageSlotData {
                                initial_value: Bytes32::from_array([0; 32]),
                                is_new_storage_slot: false, // Lie about slot existence
                            }
                        };

                    AirbenderCodecV0::encode(&slot_data)
                        .map_err(|_| internal_error!("encode slot_data failed"))
                }
                _ if query_id == PreviousIndexQuery::QUERY_ID => {
                    let key: <PreviousIndexQuery as SimpleOracleQuery>::Input =
                        AirbenderCodecV0::decode(input).map_err(|_| {
                            internal_error!("decode PreviousIndexQuery input failed")
                        })?;
                    let prev_index = self.storage.prev_tree_index(key);
                    AirbenderCodecV0::encode(&prev_index)
                        .map_err(|_| internal_error!("encode prev_index failed"))
                }
                _ if query_id == ExactIndexQuery::QUERY_ID => {
                    let key: <ExactIndexQuery as SimpleOracleQuery>::Input =
                        AirbenderCodecV0::decode(input)
                            .map_err(|_| internal_error!("decode ExactIndexQuery input failed"))?;
                    let index = self
                        .storage
                        .tree_index(key)
                        .expect("Reading index for key that is not in the tree");
                    AirbenderCodecV0::encode(&index)
                        .map_err(|_| internal_error!("encode tree index failed"))
                }
                _ if query_id == PROOF_FOR_INDEX_QUERY_ID => {
                    let index: u64 = AirbenderCodecV0::decode(input)
                        .map_err(|_| internal_error!("decode proof index failed"))?;
                    let proof = ValueAtIndexProof {
                        proof: ExistingReadProof {
                            existing: self.storage.merkle_proof(index),
                        },
                    };
                    AirbenderCodecV0::encode(&proof)
                        .map_err(|_| internal_error!("encode proof failed"))
                }
                _ => unreachable!(),
            }
        }
    }

    /// Helper: builds a CustomOracleFactory with FalseExistingSlotResponder.
    fn false_existing_slot_factory() -> CustomOracleFactory<
        impl Fn(
            BlockMetadataFromOracle,
            InMemoryTree<false>,
            InMemoryPreimageSource,
            TxListSource,
            Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            Option<DACommitmentScheme>,
        ) -> ZkEENonDeterminismSource,
    > {
        CustomOracleFactory(
            |block_metadata,
             state_tree,
             preimage_source,
             tx_source,
             proof_data,
             da_commitment_scheme| {
                build_oracle(block_metadata, proof_data, da_commitment_scheme, |oracle| {
                    oracle.add_external_processor(TxDataResponder {
                        tx_source,
                        next_tx: None,
                        next_tx_format: None,
                        next_tx_from: None,
                    });
                    oracle.add_external_processor(GenericPreimageResponder { preimage_source });
                    oracle.add_external_processor(FalseExistingSlotResponder::new(state_tree));
                })
            },
        )
    }

    /// Verifies that a malicious oracle claiming new slots are existing (is_new=false)
    /// is caught by the tree index lookup.
    #[test]
    #[should_panic(expected = "expected existing leaf for key")]
    fn test_malicious_oracle_false_existing_slot_detected() {
        let mut tester = TestingFramework::new().with_run_config(rig::run_config::forward_only());
        let wallet = tester.random_signer();

        let contract_address =
            rig::alloy::primitives::address!("1000000000000000000000000000000000000001");

        // Simple storage contract: SSTORE(0, calldata[0..32])
        let store_bytecode = hex::decode("600035600055005050").unwrap();

        tester = tester
            .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64))
            .with_evm_contract(contract_address, &store_bytecode);

        tester = tester.with_custom_oracle_factory(false_existing_slot_factory());

        let calldata = [0u8; 32];
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

        let _result = tester.execute_block(vec![tx]);
    }

    // ---- Corrupted preimage responder ----

    /// Oracle query processor that returns corrupted preimage data for targeted hashes.
    struct CorruptedPreimageResponder {
        preimage_source: InMemoryPreimageSource,
        /// Hashes for which preimage data will be corrupted (bytes XOR'd with 0xFF)
        corrupted_hashes: Vec<Bytes32>,
    }

    impl CorruptedPreimageResponder {
        fn new(preimage_source: InMemoryPreimageSource, corrupted_hashes: Vec<Bytes32>) -> Self {
            Self {
                preimage_source,
                corrupted_hashes,
            }
        }
    }

    impl CorruptedPreimageResponder {
        const SUPPORTED_QUERY_IDS: &[u32] = &[
            rig::basic_system::system_implementation::flat_storage_model::FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID,
            rig::basic_system::system_implementation::ethereum_storage_model::ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID,
            rig::basic_system::system_implementation::ethereum_storage_model::ETHEREUM_BYTECODE_PREIMAGE_QUERY_ID,
            rig::basic_system::system_implementation::ethereum_storage_model::ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID,
            rig::basic_system::system_implementation::ethereum_storage_model::ETHEREUM_MPT_PREIMAGE_WORDS_QUERY_ID,
        ];
    }

    impl OracleQueryProcessor for CorruptedPreimageResponder {
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
            _memory: &dyn RamPeek,
        ) -> Result<Vec<u8>, InternalError> {
            assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

            let hash: Bytes32 = AirbenderCodecV0::decode(input)
                .map_err(|_| internal_error!("decode hash failed"))?;

            let is_corrupted = self.corrupted_hashes.iter().any(|h| *h == hash);

            let preimage = if hash.is_zero() {
                vec![]
            } else {
                let mut data = self.preimage_source.get_preimage(hash).unwrap_or_else(|| {
                    panic!(
                        "must know a preimage for hash {} for query ID 0x{:016x}",
                        hex::encode(hash.as_u8_array_ref()),
                        query_id
                    )
                });
                if is_corrupted && !data.is_empty() {
                    data[0] ^= 0xFF;
                }
                data
            };

            use rig::basic_system::system_implementation::ethereum_storage_model::{
                ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID,
                ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID,
            };
            if query_id == ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID
                || query_id == ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID
            {
                let len = preimage.len() as u32;
                AirbenderCodecV0::encode(&len)
                    .map_err(|_| internal_error!("encode preimage length failed"))
            } else {
                AirbenderCodecV0::encode(&preimage)
                    .map_err(|_| internal_error!("encode preimage failed"))
            }
        }
    }

    /// Helper: builds a CustomOracleFactory with CorruptedPreimageResponder.
    fn corrupted_preimage_factory(
        corrupted_hashes: Vec<Bytes32>,
    ) -> CustomOracleFactory<
        impl Fn(
            BlockMetadataFromOracle,
            InMemoryTree<false>,
            InMemoryPreimageSource,
            TxListSource,
            Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            Option<DACommitmentScheme>,
        ) -> ZkEENonDeterminismSource,
    > {
        CustomOracleFactory(
            move |block_metadata,
                  state_tree,
                  preimage_source,
                  tx_source,
                  proof_data,
                  da_commitment_scheme| {
                build_oracle(block_metadata, proof_data, da_commitment_scheme, |oracle| {
                    oracle.add_external_processor(TxDataResponder {
                        tx_source,
                        next_tx: None,
                        next_tx_format: None,
                        next_tx_from: None,
                    });
                    oracle.add_external_processor(CorruptedPreimageResponder::new(
                        preimage_source,
                        corrupted_hashes.clone(),
                    ));
                    oracle.add_external_processor(ReadTreeResponder { tree: state_tree });
                })
            },
        )
    }

    /// Verifies that corrupted preimage data (hash mismatch) is detected in debug mode.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn test_corrupted_bytecode_preimage_detected_in_debug() {
        let mut tester = TestingFramework::new().with_run_config(rig::run_config::forward_only());
        let wallet = tester.random_signer();

        let contract_address =
            rig::alloy::primitives::address!("1000000000000000000000000000000000000001");

        let simple_bytecode = hex::decode("60006000f3").unwrap();

        tester = tester
            .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64))
            .with_evm_contract(contract_address, &simple_bytecode);

        let account_props = tester.get_account_properties(&contract_address);
        let bytecode_hash = account_props.bytecode_hash;

        tester = tester.with_custom_oracle_factory(corrupted_preimage_factory(vec![bytecode_hash]));

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
}

mod callable_oracle_tests {
    //! Tests for callable oracle processors (ModExp arithmetic, Blob KZG commitment).
    //!
    //! These tests validate the callable oracle processors themselves using
    //! a TestMemorySource (BTreeMap-based RamPeek impl).

    use rig::callable_oracles::blob_kzg_commitment::blob_kzg_commitment_and_proof;
    use rig::oracle_provider::airbender_codec::{AirbenderCodec, AirbenderCodecV0};
    use rig::oracle_provider::{OracleQueryProcessor, RamPeek};

    use rig::alloy::consensus::TxEip2930;
    use rig::alloy::primitives::{TxKind, U256};
    use rig::basic_system::system_functions::modexp::MODEXP_ADVICE_QUERY_ID;
    use rig::basic_system::system_implementation::flat_storage_model::{
        FlatStorageCommitment, TREE_HEIGHT,
    };
    use rig::callable_oracles::arithmetic::ArithmeticQuery;
    use rig::callable_oracles::test_utils::TestMemorySource;
    use rig::forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree};
    use rig::oracle_provider::ZkEENonDeterminismSource;
    use rig::zk_ee::common_structs::{da_commitment_scheme::DACommitmentScheme, ProofData};
    use rig::zk_ee::internal_error;
    use rig::zk_ee::system::errors::internal::InternalError;
    use rig::zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle;
    use rig::zksync_os_interface::traits::TxListSource;
    use rig::{common_target_address, TestingFramework};
    use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

    /// A malicious arithmetic oracle that returns deliberately wrong division results.
    #[derive(Default)]
    struct MaliciousArithmeticQuery {
        inner: ArithmeticQuery,
    }

    impl OracleQueryProcessor for MaliciousArithmeticQuery {
        fn supported_query_ids(&self) -> Vec<u32> {
            self.inner.supported_query_ids()
        }

        fn process(
            &mut self,
            query_id: u32,
            input: &[u8],
            memory: &dyn RamPeek,
        ) -> Result<Vec<u8>, InternalError> {
            // Get the correct result first
            let correct_bytes = self.inner.process(query_id, input, memory)?;

            // Corrupt the response by flipping the last byte (always part of data)
            let mut corrupted_bytes = correct_bytes;
            if let Some(last) = corrupted_bytes.last_mut() {
                *last ^= 0x01;
            }
            Ok(corrupted_bytes)
        }
    }

    /// Helper: builds a CustomOracleFactory with MaliciousArithmeticQuery.
    fn malicious_callable_oracle_factory() -> super::custom_oracle_factories::CustomOracleFactory<
        impl Fn(
            BlockMetadataFromOracle,
            InMemoryTree<false>,
            InMemoryPreimageSource,
            TxListSource,
            Option<ProofData<FlatStorageCommitment<{ TREE_HEIGHT }>>>,
            Option<DACommitmentScheme>,
        ) -> ZkEENonDeterminismSource,
    > {
        super::custom_oracle_factories::CustomOracleFactory(
            |block_metadata,
             state_tree,
             preimage_source,
             tx_source,
             proof_data,
             da_commitment_scheme| {
                super::custom_oracle_factories::build_oracle(
                    block_metadata,
                    proof_data,
                    da_commitment_scheme,
                    |oracle| {
                        super::custom_oracle_factories::add_standard_processors(
                            oracle,
                            state_tree,
                            preimage_source,
                            tx_source,
                        );
                        oracle.add_external_processor(MaliciousArithmeticQuery::default());
                    },
                )
            },
        )
    }

    /// Test that the MaliciousArithmeticQuery actually produces wrong results.
    #[test]
    fn test_malicious_arithmetic_query_corrupts_output() {
        let params_addr: u32 = 0x100;
        let a_addr: u32 = 0x200;
        let m_addr: u32 = 0x400;

        let mut memory = TestMemorySource::default();

        // ModExpAdviceParams: 10 / 3
        memory.insert_u32(params_addr, 0); // op
        memory.insert_u32(params_addr + 4, a_addr); // a_ptr
        memory.insert_u32(params_addr + 8, 1); // a_len (1 digit)
        memory.insert_u32(params_addr + 12, 0); // b_ptr
        memory.insert_u32(params_addr + 16, 0); // b_len
        memory.insert_u32(params_addr + 20, m_addr); // modulus_ptr
        memory.insert_u32(params_addr + 24, 1); // modulus_len

        // dividend = 10
        memory.insert_u32(a_addr, 10);
        // modulus = 3
        memory.insert_u32(m_addr, 3);

        // Build the input bytes as the RISC-V oracle expects: a pointer to params
        let input_bytes = AirbenderCodecV0::encode(&params_addr).unwrap();

        // Get correct result
        let mut correct_oracle = ArithmeticQuery::default();
        let correct_bytes = correct_oracle
            .process(MODEXP_ADVICE_QUERY_ID, &input_bytes, &memory)
            .unwrap();

        // Get malicious result
        let mut malicious_oracle = MaliciousArithmeticQuery::default();
        let malicious_bytes = malicious_oracle
            .process(MODEXP_ADVICE_QUERY_ID, &input_bytes, &memory)
            .unwrap();

        // Verify the malicious oracle corrupted the output
        assert_ne!(
            correct_bytes, malicious_bytes,
            "Malicious oracle should produce different output from correct oracle"
        );
    }

    /// Integration test: register a malicious callable oracle factory and execute a block.
    #[test]
    fn test_malicious_callable_oracle_factory_forward_mode() {
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

        tester = tester
            .with_custom_oracle_factory(malicious_callable_oracle_factory())
            .with_run_config(rig::run_config::forward_only());

        let result = tester.execute_block_no_panic(vec![tx]);
        assert!(
            result.is_ok(),
            "Forward mode should not depend on callable oracle correctness"
        );
    }

    /// Test that the blob_kzg_commitment_and_proof function produces valid output.
    #[test]
    fn test_blob_kzg_commitment_computation_consistency() {
        let data = b"test blob data for commitment verification";
        let result = blob_kzg_commitment_and_proof(data);

        assert_eq!(
            result.commitment.len(),
            48,
            "KZG commitment should be 48 bytes"
        );
        assert_eq!(result.proof.len(), 48, "KZG proof should be 48 bytes");

        let result2 = blob_kzg_commitment_and_proof(data);
        assert_eq!(result.commitment, result2.commitment);
        assert_eq!(result.proof, result2.proof);
    }
}

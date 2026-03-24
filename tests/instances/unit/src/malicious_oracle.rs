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

//!
//! Test for the block header.
//!
#![cfg(test)]

use basic_bootloader::bootloader::block_header::EMPTY_OMMER_ROOT_HASH;
use basic_bootloader::bootloader::constants::MAX_BLOCK_GAS_LIMIT;
use rig::ruint::aliases::{B160, U256};
use rig::utils::run_block_of_erc20;
use rig::{zk_ee::utils::Bytes32, Chain};

// Run a block of ERC20 transactions and check invariants on the block header.
#[test]
fn test_block_header_invariants() {
    let mut chain = Chain::empty(None);
    let output = run_block_of_erc20::<false>(&mut chain, 10, None);
    let header = output.header;

    // Check invariants on header for genesis block.
    assert_eq!(header.parent_hash, Bytes32::ZERO);
    assert_eq!(header.ommers_hash, Bytes32::from(EMPTY_OMMER_ROOT_HASH));
    assert_eq!(header.beneficiary, B160::ZERO);
    assert_eq!(header.state_root, Bytes32::ZERO);
    // TODO: enable when this is implemented
    // assert_ne!(header.transactions_root, Bytes32::ZERO);
    // assert_ne!(header.receipts_root, Bytes32::ZERO);
    assert_eq!(header.number, 0);
    assert_eq!(header.gas_limit, MAX_BLOCK_GAS_LIMIT);
    assert!(
        header.gas_used
            == output
                .tx_results
                .into_iter()
                .map(|r| r.map(|o| o.gas_used).unwrap_or_default())
                .sum::<u64>()
    );
    assert_eq!(header.timestamp, 42);
    assert_eq!(header.base_fee_per_gas, 1000);

    let genesis_hash = header.hash();

    // Run second block and check invariants.
    // Test some non-default block context values too.
    let timestamp = 43;
    let eip1559_basefee = U256::from(900);
    let coinbase = B160::from_be_bytes([1u8; 20]);
    let gas_limit = 30_000_000;

    let block_context = rig::BlockContext {
        timestamp,
        eip1559_basefee,
        coinbase,
        gas_limit,
        ..Default::default()
    };
    let output = run_block_of_erc20::<false>(&mut chain, 10, Some(block_context));
    let header = output.header;
    assert_eq!(header.parent_hash, Bytes32::from_array(genesis_hash));
    assert_eq!(header.ommers_hash, Bytes32::from(EMPTY_OMMER_ROOT_HASH));
    assert_eq!(header.beneficiary, coinbase);
    assert_eq!(header.state_root, Bytes32::ZERO);
    // TODO: enable when this is implemented
    // assert_ne!(header.transactions_root, Bytes32::ZERO);
    // assert_ne!(header.receipts_root, Bytes32::ZERO);
    assert_eq!(header.number, 1);
    assert_eq!(header.gas_limit, gas_limit);
    assert!(
        header.gas_used
            == output
                .tx_results
                .into_iter()
                .map(|r| r.map(|o| o.gas_used).unwrap_or_default())
                .sum::<u64>()
    );
    assert_eq!(header.timestamp, 43);
    assert_eq!(U256::from(header.base_fee_per_gas), eip1559_basefee);
}

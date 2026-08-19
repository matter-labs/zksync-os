//! Per-transaction memory budget of the block-scoped raw preimage cache.
//!
//! Raw preimages stay in the cache for the whole block and survive the rollback
//! of an invalidated transaction. A later transaction must still pay for them
//! against its own memory budget: the proving run executes only the included
//! transactions, so in that run the later transaction is the first one to
//! materialize those preimages. See the charging invariant in
//! `docs/system/io/caches.md`. Only the preimages of an accepted transaction
//! are free for the transactions that follow.
//!
//! Both tests execute the same three transactions and differ only in the block
//! pubdata limit. The limit decides whether the bootloader accepts or drops the
//! second transaction after it warms a large set of bytecodes. The third
//! transaction warms the same set plus two more bytecodes. That total crosses
//! the per-transaction budget only when the bootloader drops the second
//! transaction.

use rig::alloy::consensus::TxEip1559;
use rig::alloy::primitives::{address, Address, TxKind, U256 as AlloyU256};
use rig::alloy::signers::local::PrivateKeySigner;
use rig::basic_system::system_implementation::flat_storage_model::MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX;
use rig::constants::*;
use rig::evm_bytecode::BytecodeBuilder;
use rig::forward_system::run::output::BlockOutput;
use rig::ruint::aliases::U256;
use rig::zk_ee::system::metadata::chain_config::DEFAULT_MAX_TX_GAS_LIMIT;
use rig::zksync_os_interface::error::InvalidTransaction;
use rig::zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;
use rig::{
    assert_gas_used_gt, assert_tx_reverted, assert_tx_success, BlockContext, TestingFramework,
};

/// Runtime code length of one filler contract. The value is a multiple of 64, so
/// ZKsync OS stores the code without padding and the jump-destination bitmap
/// covers it exactly.
const FILLER_CODE_LEN: usize = 8 * 1024 * 1024;

/// Jump-destination bitmap that ZKsync OS appends to stored bytecode: one bit
/// per code byte.
const FILLER_ARTIFACTS_LEN: usize = FILLER_CODE_LEN / 8;

/// Preimage that one filler contract retains in the cache: code plus artifacts.
const FILLER_PREIMAGE_LEN: usize = FILLER_CODE_LEN + FILLER_ARTIFACTS_LEN;

/// Filler contracts that both the second and the third transaction warm.
const SHARED_FILLER_COUNT: u16 = 13;

/// Filler contracts that only the third transaction warms.
const EXTRA_FILLER_COUNT: u16 = 2;

/// The shared preimages alone must fit in one transaction budget. The margin of
/// one whole entry absorbs the per-entry bookkeeping that the cache adds on top
/// of every preimage, plus the account and toucher preimages of the transaction.
const _: () = assert!(
    (SHARED_FILLER_COUNT as usize + 1) * FILLER_PREIMAGE_LEN
        < MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX
);

/// A second charge for the shared preimages must exceed one transaction budget
/// on preimage bytes alone.
const _: () = assert!(
    (SHARED_FILLER_COUNT + EXTRA_FILLER_COUNT) as usize * FILLER_PREIMAGE_LEN
        > MAX_PREIMAGE_CACHE_BYTES_ADDED_PER_TX
);

/// First address of the contiguous filler contract range.
const FILLER_ADDRESS_BASE: Address = address!("00000000000000000000000000000000f1110000");

/// Contract that the second transaction calls.
const SHARED_TOUCHER_ADDRESS: Address = address!("00000000000000000000000000000000f0110001");

/// Contract that the third transaction calls.
const FULL_TOUCHER_ADDRESS: Address = address!("00000000000000000000000000000000f0110002");

/// Recipient of the first transaction.
const TRANSFER_RECIPIENT_ADDRESS: Address = address!("00000000000000000000000000000000f0110003");

/// Gas limit of the two decommitting transactions. Each one charges about 750
/// million native for its decommits, which needs about 7.5 million gas at the
/// default native price.
const TOUCHER_GAS_LIMIT: u64 = DEFAULT_MAX_TX_GAS_LIMIT;

/// Fresh storage slots that the second transaction writes. They are the only
/// large pubdata producer in the block, which lets the block pubdata limit
/// decide whether the bootloader accepts that transaction.
const SHARED_TOUCHER_STORAGE_WRITES: u16 = 100;

/// Block pubdata limit that drops the second transaction.
///
/// The bootloader compares the limit against the block total, which starts at
/// `BLOCK_INTRINSIC_PUBDATA_BYTES` (118 bytes). Measured block totals, in
/// bytes: 292 after the first transaction, 3_867 after the second, and 467
/// after the third one once the bootloader drops the second one. The limit sits
/// above what the first and third transaction need and below what the second
/// one reaches.
const PUBDATA_LIMIT_THAT_DROPS_SHARED_TOUCHER: u64 = 2_000;

/// Address of the filler contract at `index`.
fn filler_address(index: u16) -> Address {
    let mut bytes = FILLER_ADDRESS_BASE.into_array();
    bytes[18..].copy_from_slice(&index.to_be_bytes());
    Address::from(bytes)
}

/// Runtime code of the filler contract at `index`.
///
/// The code never executes. The trailing index keeps the bytecodes distinct, so
/// every contract retains its own cache entry.
fn filler_bytecode(index: u16) -> Vec<u8> {
    let mut bytecode = vec![0u8; FILLER_CODE_LEN];
    bytecode[FILLER_CODE_LEN - 2..].copy_from_slice(&index.to_be_bytes());
    bytecode
}

/// Builds runtime code that decommits the bytecode of every filler contract
/// below `filler_count`, then writes `storage_writes` fresh storage slots.
///
/// A zero-length `EXTCODECOPY` still decommits the whole target bytecode, so one
/// call retains one cache entry per filler contract and copies nothing.
fn toucher_bytecode(filler_count: u16, storage_writes: u16) -> Vec<u8> {
    let mut builder = BytecodeBuilder::new();
    for index in 0..filler_count {
        // EXTCODECOPY pops address, destination offset, source offset, length.
        builder = builder
            .push0()
            .push0()
            .push0()
            .push_address(filler_address(index))
            .extcodecopy();
    }
    for slot in 1..=storage_writes {
        // SSTORE pops key, then value. Both are the slot index, which is never
        // zero, so every write produces a state diff.
        builder = builder.push_u16(slot).push_u16(slot).sstore();
    }
    builder.return_empty().finish()
}

fn call_tx(signer: PrivateKeySigner, to: Address, gas_limit: u64) -> ZKsyncTxEnvelope {
    let tx = TxEip1559 {
        chain_id: TEST_CHAIN_ID,
        nonce: 0,
        max_fee_per_gas: DEFAULT_MAX_FEE,
        max_priority_fee_per_gas: DEFAULT_PRIORITY_FEE,
        gas_limit,
        to: TxKind::Call(to),
        value: AlloyU256::ZERO,
        access_list: Default::default(),
        input: Default::default(),
    };
    ZKsyncTxEnvelope::from_eth_tx(tx, signer)
}

/// Executes one block of three transactions under `pubdata_limit`.
///
/// Transaction 0 is a plain transfer. Transaction 1 warms the shared filler
/// bytecodes and writes storage. Transaction 2 warms the shared filler
/// bytecodes plus the extra ones. Each transaction has its own sender, so the
/// nonces stay independent.
fn execute_block_with_pubdata_limit(pubdata_limit: u64) -> BlockOutput {
    let transfer_signer = PrivateKeySigner::random();
    let shared_toucher_signer = PrivateKeySigner::random();
    let full_toucher_signer = PrivateKeySigner::random();

    let mut tester = TestingFramework::new()
        // The block retains more than 128 MiB of raw preimages. Replaying that
        // in the RISC-V simulator is not viable, so this test stays on the
        // forward path.
        .with_run_config(rig::run_config::forward_only())
        // REVM has no preimage-cache memory budget and prices decommits
        // differently, so it diverges from ZKsync OS on both blocks by design.
        .without_revm_consistency_check()
        .with_evm_contract(
            SHARED_TOUCHER_ADDRESS,
            &toucher_bytecode(SHARED_FILLER_COUNT, SHARED_TOUCHER_STORAGE_WRITES),
        )
        .with_evm_contract(
            FULL_TOUCHER_ADDRESS,
            &toucher_bytecode(SHARED_FILLER_COUNT + EXTRA_FILLER_COUNT, 0),
        )
        .with_balance(transfer_signer.address(), U256::from(DEFAULT_BALANCE))
        .with_balance(shared_toucher_signer.address(), U256::from(DEFAULT_BALANCE))
        .with_balance(full_toucher_signer.address(), U256::from(DEFAULT_BALANCE))
        .with_block_context(BlockContext {
            pubdata_limit,
            ..Default::default()
        });

    for index in 0..SHARED_FILLER_COUNT + EXTRA_FILLER_COUNT {
        tester.set_evm_contract(filler_address(index), &filler_bytecode(index));
    }

    tester.execute_block(vec![
        call_tx(
            transfer_signer,
            TRANSFER_RECIPIENT_ADDRESS,
            TRANSFER_GAS_LIMIT,
        ),
        call_tx(
            shared_toucher_signer,
            SHARED_TOUCHER_ADDRESS,
            TOUCHER_GAS_LIMIT,
        ),
        call_tx(full_toucher_signer, FULL_TOUCHER_ADDRESS, TOUCHER_GAS_LIMIT),
    ])
}

#[test]
fn preimage_budget_recounts_entries_from_invalidated_tx() {
    let output = execute_block_with_pubdata_limit(PUBDATA_LIMIT_THAT_DROPS_SHARED_TOUCHER);

    assert_tx_success!(output, 0);
    assert!(
        matches!(
            &output.tx_results[1],
            Err(InvalidTransaction::BlockPubdataLimitReached)
        ),
        "expected BlockPubdataLimitReached, got {:?}",
        output.tx_results[1]
    );
    // The shared bytecodes stay in the cache but belong to a dropped
    // transaction, so this transaction pays for them again and runs out of
    // native resources on the extra bytecodes.
    assert_tx_reverted!(output, 2);
    // A lack of native resources exhausts the gas limit, which separates this
    // halt from an ordinary revert. The control test proves that the same work
    // fits well inside the same gas limit.
    assert_gas_used_gt!(output, 2, TOUCHER_GAS_LIMIT - 1);
}

#[test]
fn preimage_budget_promotes_entries_from_accepted_tx() {
    let output = execute_block_with_pubdata_limit(u64::MAX);

    assert_tx_success!(output, 0);
    assert_tx_success!(output, 1);
    // In this block the shared bytecodes belong to an accepted transaction, so
    // this transaction pays only for the extra bytecodes and stays in budget.
    assert_tx_success!(output, 2);
}

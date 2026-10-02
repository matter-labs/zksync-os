//! Guest heap headroom at the end of a full block in the proving run.
//!
//! At block end the proving run checks a Merkle proof for every storage slot
//! that the block accessed. It keeps all of them in memory while it computes
//! the new state root. A read of an empty slot needs proofs for both
//! neighbouring leaves. The forward run builds none of these proofs, so no
//! forward-side limit bounds them, only block gas does. The block-scoped
//! preimage cache, which can retain up to `MAX_PREIMAGE_CACHE_RETAINED_BYTES`,
//! is still alive at that point too.
//!
//! The block below stays within the production block gas limit and within
//! every per-transaction limit. It first fills the preimage cache close to its
//! cap by calling many distinct contracts of the maximum code size. Then it
//! spends the remaining gas on cold reads of empty slots. The proving run of
//! this block used to run out of guest heap in the post-op, because the
//! bootloader kept its EVM heap and return-data buffers until the post-op
//! returned.
//!
//! The forward run always passes. The test only checks the guest heap when the
//! rig runs the RISC-V binary (`ZKSYNC_RISC_V_RUN` or `CI`).

use rig::alloy::consensus::TxEip1559;
use rig::alloy::primitives::{address, Address, TxKind, U160, U256 as AlloyU256};
use rig::alloy::signers::local::PrivateKeySigner;
use rig::basic_system::system_implementation::flat_storage_model::MAX_PREIMAGE_CACHE_RETAINED_BYTES;
use rig::constants::*;
use rig::evm_bytecode::BytecodeBuilder;
use rig::ruint::aliases::{B256, U256};
use rig::zk_ee::system::metadata::chain_config::{DEFAULT_MAX_CODE_SIZE, DEFAULT_MAX_TX_GAS_LIMIT};
use rig::zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;
use rig::{BlockContext, TestingFramework};

/// Block gas limit that the sequencer uses in production.
const PRODUCTION_BLOCK_GAS_LIMIT: u64 = 100_000_000;

/// Gas limit of every transaction in the block.
const TX_GAS_LIMIT: u64 = 16_000_000;
const _: () = assert!(TX_GAS_LIMIT <= DEFAULT_MAX_TX_GAS_LIMIT);

/// Leaves added to the state tree before the block.
///
/// Flat storage keys are hashes, so reads of empty slots land in random gaps of
/// the tree. The post-op keeps one proof per distinct leaf. The tree has to be
/// large enough that most reads land in different gaps and need proofs for
/// different neighbours.
const TREE_FILLER_LEAF_COUNT: u64 = 500_000;

/// Contracts that the block calls to fill the preimage cache.
const FILLER_CONTRACT_COUNT: u64 = 9_000;

/// Filler contracts that one transaction calls. Every call decommits the whole
/// callee code, and about 4,500 calls use up the native budget of one
/// transaction.
const FILLER_CALLS_PER_TX: u64 = 1_500;

/// Code length of one filler contract: the deployed-code limit of the default
/// chain config. It is a multiple of 64, so the code is stored without padding.
const FILLER_CODE_LEN: usize = DEFAULT_MAX_CODE_SIZE as usize;

/// Preimage that one filler contract retains in the cache: code, plus the
/// jump-destination bitmap of one bit per code byte, plus the per-entry
/// overhead that the cache accounts for.
const FILLER_PREIMAGE_LEN: usize = FILLER_CODE_LEN + FILLER_CODE_LEN / 8 + 256;

/// The filler contracts must fit in the preimage cache, or the bootloader
/// drops the transactions that call them.
const _: () = assert!(
    FILLER_CONTRACT_COUNT as usize * FILLER_PREIMAGE_LEN < MAX_PREIMAGE_CACHE_RETAINED_BYTES
);

/// Transactions that read empty slots.
const EMPTY_READ_TX_COUNT: u64 = 6;

/// Cold reads of empty slots that one transaction makes. Together with the
/// filler calls they use almost all of the block gas.
const EMPTY_READS_PER_TX: u64 = 4_800;

/// Account that holds the storage slots filling the state tree.
const TREE_FILLER_ADDRESS: Address = address!("00000000000000000000000000000000f0120001");

/// Contract that calls the filler contracts.
const FILLER_CALLER_ADDRESS: Address = address!("00000000000000000000000000000000f0120002");

/// Contract that reads its own empty storage slots.
const EMPTY_READER_ADDRESS: Address = address!("00000000000000000000000000000000f0120003");

/// Address of the first filler contract. The filler contracts occupy a
/// contiguous range above the special address space, so the caller can derive
/// the callee address from the loop counter.
const FILLER_ADDRESS_BASE: u64 = 0xf121_0000;

/// Offset of the loop head in the loop contracts: the length of the prologue
/// `PUSH0 CALLDATALOAD PUSH1 0x20 CALLDATALOAD`.
const LOOP_HEAD: u8 = 5;

const JUMPDEST: u8 = 0x5b;

fn filler_address(index: u64) -> Address {
    Address::from(U160::from(FILLER_ADDRESS_BASE + index))
}

/// Runtime code of the filler contract at `index`.
///
/// The code stops at its first byte. The trailing index keeps the bytecodes
/// distinct, so every contract retains its own cache entry.
fn filler_bytecode(index: u64) -> Vec<u8> {
    let mut bytecode = vec![0u8; FILLER_CODE_LEN];
    bytecode[FILLER_CODE_LEN - 8..].copy_from_slice(&index.to_be_bytes());
    bytecode
}

/// Builds runtime code that runs `body` once for every `i` in `[start, end)`.
/// Calldata holds `end` in word 0 and `start` in word 1, see `range_calldata`.
/// The stack is `[end, i]` when `body` starts and must be the same when it
/// ends. The range must not be empty.
fn range_loop_bytecode(body: impl FnOnce(BytecodeBuilder) -> BytecodeBuilder) -> Vec<u8> {
    let head = BytecodeBuilder::new()
        .push0()
        .calldataload()
        .push_u8(0x20)
        .calldataload()
        .jumpdest();
    let bytecode = body(head)
        // i += 1, then jump back while i < end.
        .push_u8(1)
        .add()
        .dup(2)
        .dup(2)
        .lt()
        .push_u8(LOOP_HEAD)
        .jumpi()
        .stop()
        .finish();
    assert_eq!(bytecode[LOOP_HEAD as usize], JUMPDEST);
    bytecode
}

/// Calls the contract at address `i` for every `i` in the calldata range,
/// forwarding all gas and passing no data.
fn filler_caller_bytecode() -> Vec<u8> {
    range_loop_bytecode(|builder| {
        // CALL pops gas, address, value, input offset and size, output offset
        // and size. After the five zeros, `i` is the sixth stack item.
        builder.push0_n(5).dup(6).gas().call().pop()
    })
}

/// Reads storage slot `i` for every `i` in the calldata range. The contract
/// never writes storage, so every read is a cold read of an empty slot.
fn empty_reader_bytecode() -> Vec<u8> {
    range_loop_bytecode(|builder| builder.dup(1).sload().pop())
}

fn range_calldata(start: u64, end: u64) -> Vec<u8> {
    [
        U256::from(end).to_be_bytes::<32>(),
        U256::from(start).to_be_bytes::<32>(),
    ]
    .concat()
}

fn call_tx(signer: &PrivateKeySigner, nonce: u64, to: Address, input: Vec<u8>) -> ZKsyncTxEnvelope {
    let tx = TxEip1559 {
        chain_id: TEST_CHAIN_ID,
        nonce,
        max_fee_per_gas: DEFAULT_MAX_FEE,
        max_priority_fee_per_gas: DEFAULT_PRIORITY_FEE,
        gas_limit: TX_GAS_LIMIT,
        to: TxKind::Call(to),
        value: AlloyU256::ZERO,
        access_list: Default::default(),
        input: input.into(),
    };
    ZKsyncTxEnvelope::from_eth_tx(tx, signer.clone())
}

#[test]
fn full_block_of_preimages_and_empty_reads_fits_in_guest_heap() {
    let mut tester = TestingFramework::new()
        .with_evm_contract(FILLER_CALLER_ADDRESS, &filler_caller_bytecode())
        .with_evm_contract(EMPTY_READER_ADDRESS, &empty_reader_bytecode())
        .with_block_context(BlockContext {
            gas_limit: PRODUCTION_BLOCK_GAS_LIMIT,
            ..Default::default()
        });
    for key in 1..=TREE_FILLER_LEAF_COUNT {
        tester.set_storage_slot(
            TREE_FILLER_ADDRESS,
            U256::from(key),
            B256::from(U256::from(key)),
        );
    }
    for index in 0..FILLER_CONTRACT_COUNT {
        tester.set_evm_contract(filler_address(index), &filler_bytecode(index));
    }
    let signer = tester.prefunded_random_signer();

    let mut transactions = Vec::new();
    for start in (0..FILLER_CONTRACT_COUNT).step_by(FILLER_CALLS_PER_TX as usize) {
        let end = (start + FILLER_CALLS_PER_TX).min(FILLER_CONTRACT_COUNT);
        let input = range_calldata(FILLER_ADDRESS_BASE + start, FILLER_ADDRESS_BASE + end);
        transactions.push(call_tx(
            &signer,
            transactions.len() as u64,
            FILLER_CALLER_ADDRESS,
            input,
        ));
    }
    for tx_index in 0..EMPTY_READ_TX_COUNT {
        let start = tx_index * EMPTY_READS_PER_TX;
        let input = range_calldata(start, start + EMPTY_READS_PER_TX);
        transactions.push(call_tx(
            &signer,
            transactions.len() as u64,
            EMPTY_READER_ADDRESS,
            input,
        ));
    }

    let output = tester.execute_block(transactions);

    tester.assert_all_txs_succeeded(&output);
    // The test only exercises the worst case if the block is close to full.
    assert!(
        output.header.gas_used > PRODUCTION_BLOCK_GAS_LIMIT / 10 * 9,
        "block must use most of its gas limit, used {}",
        output.header.gas_used
    );
}

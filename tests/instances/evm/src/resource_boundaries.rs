//! Ergs and native budgets at and around multiples of 2^31. The interpreter keeps the low 31 bits
//! of each in a register during a run and the rest in the frame, so these cases cover the
//! boundaries of that split with real instructions: a charge that borrows from the high part (also
//! at a low part of 0), a charge of exactly 2^31 ergs, exact exhaustion, the GAS opcode and the
//! SSTORE stipend check (which read the whole amount), memory growth after the step charge, CALL
//! and CREATE forwarding budgets across 2^31, budgets far above 2^32, and native at 2^31 and at
//! the free-native limit.
//!
//! Each case pins the status, gas and native of every transaction and the storage the test
//! contracts write to `resource_boundaries.golden`, captured with the interpreter before the split
//! (at b674191e). None of the native-visible cases lands a jump, since a landing charges 20 native
//! less on the proving target (see `push2_jump_fusion`).

use crate::test_support::{call_tx_with, new_tester};
use rig::alloy::primitives::{address, Address, B256, U256 as AlloyU256};
use rig::alloy::signers::local::PrivateKeySigner;
use rig::constants::{DEFAULT_BALANCE, DEFAULT_MAX_FEE, DEFAULT_PRIORITY_FEE, TEST_CHAIN_ID};
use rig::ruint::aliases::U256;
use rig::zksync_os_interface::types::ExecutionResult;
use rig::zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;
use rig::{testing_signer, BlockContext, BlockOutput, TestingFramework};

const DRIVER: Address = address!("0000000000000000000000000000000000000f10");
const CALLEE: Address = address!("0000000000000000000000000000000000000f11");
const LEAF: Address = address!("0000000000000000000000000000000000000f12");

/// 2^31 ergs
const SPLIT_GAS: u64 = 1 << 23;

/// Cap on the gas of a transaction, raised for the budgets above 2^32 ergs and native
const MAX_TX_GAS: u64 = 1 << 33;

/// Gas of the budgets that don't depend on native
const PLENTY: u64 = 1 << 20;

/// Calls the address in calldata word 1 with the gas in word 0, keeping 32 bytes of return
/// data, then stores the call's success flag, the gas left, the return data size and the first
/// returned word in slots 0 to 3
fn driver_code() -> Vec<u8> {
    vec![
        0x60, 0x20, 0x5f, 0x5f, 0x5f, 0x5f, // CALL's sizes, offsets and value
        0x60, 0x20, 0x35, // the callee
        0x5f, 0x35, // the gas
        0xf1, // CALL
        0x5f, 0x55, // SSTORE(0, success)
        0x5a, 0x60, 0x01, 0x55, // SSTORE(1, GAS)
        0x3d, 0x60, 0x02, 0x55, // SSTORE(2, RETURNDATASIZE)
        0x5f, 0x51, 0x60, 0x03, 0x55, // SSTORE(3, MLOAD(0))
        0x00,
    ]
}

/// `GAS PUSH1 slot SSTORE`
fn store_gas(code: &mut Vec<u8>, slot: u8) {
    code.extend_from_slice(&[0x5a, 0x60, slot, 0x55]);
}

/// `GAS PUSH0 MSTORE PUSH1 32 PUSH0 RETURN`: returns the gas left
fn return_gas(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x5a, 0x5f, 0x52, 0x60, 0x20, 0x5f, 0xf3]);
}

/// Stores the gas left twice and returns it
fn gas_recorder() -> Vec<u8> {
    let mut code = Vec::new();
    store_gas(&mut code, 0);
    store_gas(&mut code, 1);
    return_gas(&mut code);
    code
}

/// A transaction of `signer` (at `nonce`) to the driver, which calls `callee` with `call_gas`
fn driver_tx(
    signer: &PrivateKeySigner,
    nonce: u64,
    gas_limit: u64,
    call_gas: u64,
    callee: Address,
) -> ZKsyncTxEnvelope {
    let mut calldata = U256::from(call_gas).to_be_bytes::<32>().to_vec();
    calldata.extend_from_slice(&[0; 12]);
    calldata.extend_from_slice(callee.as_slice());
    call_tx_with(
        signer.clone(),
        DRIVER,
        nonce,
        gas_limit,
        AlloyU256::ZERO,
        calldata,
        DEFAULT_MAX_FEE,
        DEFAULT_PRIORITY_FEE,
        TEST_CHAIN_ID,
    )
}

/// A chain with the driver, `contracts` and the testing signer's balance
fn tester(contracts: &[(Address, &[u8])], context: BlockContext) -> TestingFramework {
    let mut tester = new_tester()
        .with_max_tx_gas_limit(MAX_TX_GAS)
        .with_block_context(context)
        .with_balance(testing_signer(0).address(), U256::from(DEFAULT_BALANCE))
        .with_evm_contract(DRIVER, &driver_code());
    for (address, code) in contracts {
        tester = tester.with_evm_contract(*address, code);
    }
    tester
}

/// Native priced at one per gas (the gas price is the base fee), so that native sets the fee
fn native_visible() -> BlockContext {
    BlockContext {
        native_price: U256::from(DEFAULT_MAX_FEE),
        ..Default::default()
    }
}

/// What a block did: per transaction its status, gas and native, then every storage slot a
/// test contract (or a contract it created) wrote, in order
fn observe(case: &str, output: &BlockOutput) -> Vec<String> {
    let mut lines = Vec::new();
    for (i, tx) in output.tx_results.iter().enumerate() {
        let line = match tx {
            Ok(tx) => {
                let status = match &tx.execution_result {
                    ExecutionResult::Success(_) => "success",
                    ExecutionResult::Revert(_) => "revert",
                };
                format!(
                    "{case} tx {i} {status} gas {} computational_native {} native {}",
                    tx.gas_used, tx.computational_native_used, tx.native_used
                )
            }
            Err(e) => format!("{case} tx {i} invalid {e:?}"),
        };
        lines.push(line);
    }
    let system = |address: &Address| address.as_slice()[..18].iter().all(|b| *b == 0);
    let mut writes: Vec<_> = output
        .storage_writes
        .iter()
        .filter(|w| [DRIVER, CALLEE, LEAF].contains(&w.account) || !system(&w.account))
        .filter(|w| w.account != testing_signer(0).address())
        .map(|w| (w.account, w.account_key, w.value))
        .collect();
    writes.sort();
    for (account, key, value) in writes {
        lines.push(format!(
            "{case} {account} [{}] = {value}",
            U256::from_be_bytes(key.0)
        ));
    }
    lines
}

/// Runs one block of driver transactions, `(gas_limit, call_gas)` each, to `callee`
fn run(
    case: &str,
    contracts: &[(Address, &[u8])],
    context: BlockContext,
    callee: Address,
    txs: &[(u64, u64)],
) -> Vec<String> {
    let signer = testing_signer(0);
    let mut tester = tester(contracts, context);
    let txs = txs
        .iter()
        .enumerate()
        .map(|(nonce, &(gas_limit, call_gas))| {
            driver_tx(&signer, nonce as u64, gas_limit, call_gas, callee)
        })
        .collect();
    let output = tester.execute_block(txs);
    observe(case, &output)
}

/// A driver transaction with enough gas to forward `call_gas`
fn forwarding(call_gas: u64) -> (u64, u64) {
    (call_gas + call_gas / 63 + PLENTY, call_gas)
}

/// Compares `lines` with the lines of the golden file that belong to `case`
fn check(case: &str, lines: &[String]) {
    let prefix = format!("{case} ");
    let golden: Vec<&str> = include_str!("resource_boundaries.golden")
        .lines()
        .filter(|l| l.starts_with(&prefix))
        .collect();
    assert!(!golden.is_empty(), "no golden lines for {case}");
    assert_eq!(lines, golden, "{case}");
}

/// What `account` wrote to `slot` in the observed block (the driver: the call's success flag
/// in 0, the gas left in 1, the return data size in 2 and the first returned word in 3)
fn slot(lines: &[String], account: Address, slot: u64) -> Option<U256> {
    let key = format!(" {account} [{slot}] = ");
    lines.iter().find_map(|l| {
        l.split_once(&key)
            .map(|(_, v)| U256::from_be_bytes(v.parse::<B256>().unwrap().0))
    })
}

/// GAS right at the start of a frame whose ergs are at, below and above multiples of 2^31: the
/// first charge (GAS, 2 gas) borrows at a low part of 0, below 2 gas, or leaves exactly 0
#[test]
fn gas_left_across_the_split() {
    let budgets = [
        SPLIT_GAS - 1,
        SPLIT_GAS,
        SPLIT_GAS + 1,
        SPLIT_GAS + 2,
        SPLIT_GAS + 3,
        2 * SPLIT_GAS,
        2 * SPLIT_GAS + 1,
        3 * SPLIT_GAS + 5,
        (1 << 27) + 1,
    ];
    for call_gas in budgets {
        let case = format!("gas_left_{call_gas}");
        let callee = gas_recorder();
        let lines = run(
            &case,
            &[(CALLEE, &callee)],
            BlockContext::default(),
            CALLEE,
            &[forwarding(call_gas)],
        );
        check(&case, &lines);
        // the case is the one it means to be: the callee started with `call_gas`
        assert_eq!(
            slot(&lines, CALLEE, 0),
            Some(U256::from(call_gas - 2)),
            "{case}"
        );
    }
}

/// The SSTORE stipend check (the gas left must be above 2300) reads the whole amount: at 2300
/// and 2301 left, and at 2^23 + x left, where the low part alone is below the stipend
#[test]
fn sstore_stipend_across_the_split() {
    // SSTORE(0, 1), then SSTORE(0, 2) (a warm write of 100 gas) with `left` gas at its check
    let callee = [0x60, 0x01, 0x5f, 0x55, 0x60, 0x02, 0x5f, 0x55, 0x00];
    let before_check = 3 + 2 + 22_100 + 3 + 2;
    for left in [
        2299,
        2300,
        2301,
        2302,
        SPLIT_GAS,
        SPLIT_GAS + 100,
        SPLIT_GAS + 2300,
        SPLIT_GAS + 2301,
    ] {
        let case = format!("sstore_stipend_{left}");
        let call_gas = before_check + left;
        let lines = run(
            &case,
            &[(CALLEE, &callee)],
            BlockContext::default(),
            CALLEE,
            &[forwarding(call_gas)],
        );
        check(&case, &lines);
        let stored = slot(&lines, CALLEE, 0);
        let expected = (left > 2300).then(|| U256::from(2));
        assert_eq!(stored, expected, "{case}");
    }
}

/// Memory growth charged right after the step charge of the instruction: a growth of exactly
/// 2^23 gas (2^31 ergs, from 119 to 64774 words) that leaves the frame with exactly nothing,
/// one gas short and one gas over; and a small growth after a step charge that borrows, or that
/// leaves a low part of 0
#[test]
fn memory_growth_across_the_split() {
    // MSTORE8 at 3807 (119 words, 384 gas), then at 2072767 (64774 words, 2^23 gas more)
    let exact = [
        0x60, 0x01, 0x61, 0x0e, 0xdf, 0x53, 0x60, 0x01, 0x62, 0x1f, 0xa0, 0xbf, 0x53, 0x00,
    ];
    let exact_gas = 9 + 384 + 9 + SPLIT_GAS;
    for call_gas in [exact_gas - 1, exact_gas, exact_gas + 1] {
        let case = format!("memory_growth_exact_{call_gas}");
        let lines = run(
            &case,
            &[(CALLEE, &exact)],
            BlockContext::default(),
            CALLEE,
            &[forwarding(call_gas)],
        );
        check(&case, &lines);
        let succeeded = (call_gas >= exact_gas).then(|| U256::from(1));
        assert_eq!(slot(&lines, DRIVER, 0), succeeded, "{case}");
    }
    // MSTORE8 at 4095 (128 words, 416 gas), then the gas left
    let mut small = vec![0x60, 0x01, 0x61, 0x0f, 0xff, 0x53];
    store_gas(&mut small, 0);
    small.push(0x00);
    for left in [SPLIT_GAS + 1, SPLIT_GAS + 2, SPLIT_GAS + 3] {
        let case = format!("memory_growth_small_{left}");
        let lines = run(
            &case,
            &[(CALLEE, &small)],
            BlockContext::default(),
            CALLEE,
            &[forwarding(6 + left)],
        );
        check(&case, &lines);
        assert_eq!(
            slot(&lines, CALLEE, 0),
            Some(U256::from(left - 3 - 416 - 2)),
            "{case}"
        );
    }
}

/// The smallest budget of which a CALL or CREATE forwards `target` (all but a 64th), given
/// what the frame spends before and on the instruction
fn budget_forwarding(target: u64, spent: u64) -> u64 {
    let available = (target..).find(|a| a - a / 64 >= target).unwrap();
    assert_eq!(available - available / 64, target);
    available + spent
}

/// A CALL of all the gas that forwards a budget at and around 2^31 ergs and 2^32 ergs, from a
/// frame whose own budget is just above it, and the gas it gets back
#[test]
fn call_forwarding_across_the_split() {
    // CALL(GAS, LEAF) keeping 32 bytes, then the success flag, the gas left and the returned word
    let mut middle = vec![0x60, 0x20, 0x5f, 0x5f, 0x5f, 0x5f, 0x73];
    middle.extend_from_slice(LEAF.as_slice());
    middle.extend_from_slice(&[0x5a, 0xf1, 0x5f, 0x55]);
    store_gas(&mut middle, 1);
    middle.extend_from_slice(&[0x5f, 0x51, 0x60, 0x02, 0x55, 0x00]);
    let leaf = gas_recorder();
    // the pushes and GAS (16), the cold account (2600) and a word of memory (3)
    let spent = 16 + 2600 + 3;
    for target in [
        SPLIT_GAS - 1,
        SPLIT_GAS,
        SPLIT_GAS + 1,
        2 * SPLIT_GAS - 1,
        2 * SPLIT_GAS,
    ] {
        let case = format!("call_forwarding_{target}");
        let call_gas = budget_forwarding(target, spent);
        let lines = run(
            &case,
            &[(CALLEE, &middle), (LEAF, &leaf)],
            BlockContext::default(),
            CALLEE,
            &[forwarding(call_gas)],
        );
        check(&case, &lines);
        assert_eq!(
            slot(&lines, LEAF, 0),
            Some(U256::from(target - 2)),
            "{case}"
        );
    }
}

/// A CREATE that forwards a budget at and around 2^31 ergs to a constructor that stores its gas
#[test]
fn create_forwarding_across_the_split() {
    // the constructor: SSTORE(0, GAS), RETURN(0, 0)
    let init = [0x5a, 0x5f, 0x55, 0x5f, 0x5f, 0xf3];
    // MSTORE(0, init) (right aligned), CREATE(0, 26, 6), SSTORE(0, address), SSTORE(1, GAS)
    let mut middle = vec![0x65];
    middle.extend_from_slice(&init);
    middle.extend_from_slice(&[0x5f, 0x52, 0x60, 0x06, 0x60, 0x1a, 0x5f, 0xf0, 0x5f, 0x55]);
    store_gas(&mut middle, 1);
    middle.push(0x00);
    // the pushes, MSTORE and its word (19), CREATE and the word of init code (32002)
    let spent = 19 + 32_002;
    for target in [SPLIT_GAS - 1, SPLIT_GAS, SPLIT_GAS + 1] {
        let case = format!("create_forwarding_{target}");
        let call_gas = budget_forwarding(target, spent);
        let lines = run(
            &case,
            &[(CALLEE, &middle)],
            BlockContext::default(),
            CALLEE,
            &[forwarding(call_gas)],
        );
        check(&case, &lines);
        let created = slot(&lines, CALLEE, 0).expect("the constructor ran");
        let created = Address::from_slice(&created.to_be_bytes::<32>()[12..]);
        assert_eq!(
            slot(&lines, created, 0),
            Some(U256::from(target - 2)),
            "{case}"
        );
    }
}

/// Native, which a transaction's frames share: straight-line code that crosses 2^31 native at
/// every alignment of one `PUSH1 POP` pair (120 native), with native setting the fee; and free
/// native, which starts just below 2^64
#[test]
fn native_across_the_split() {
    // 4.8M native of pairs, more than the rest of the transaction (about 2M, and 6 per byte of
    // this code), so that the middle of the transaction's native is in the pairs
    let mut callee = Vec::new();
    for _ in 0..40_000 {
        callee.extend_from_slice(&[0x60, 0x01, 0x50]);
    }
    store_gas(&mut callee, 0);
    callee.push(0x00);
    let contracts: &[(Address, &[u8])] = &[(CALLEE, &callee)];
    // the native the transaction uses, which the body dominates (forward only: the cases
    // below are the check)
    let mut probe = tester(contracts, native_visible());
    probe.set_run_config(Some(rig::run_config::forward_only()));
    let signer = testing_signer(0);
    let output = probe.execute_block(vec![driver_tx(&signer, 0, 1 << 32, 1 << 40, CALLEE)]);
    let used = output.tx_results[0]
        .as_ref()
        .unwrap()
        .computational_native_used;
    // a gas limit that puts 2^31 native halfway through the body, then every alignment
    let middle = (1 << 31) + used / 2;
    let txs: Vec<_> = (0..120).map(|d| (middle + d, 1 << 40)).collect();
    let lines = run("native", contracts, native_visible(), CALLEE, &txs);
    check("native", &lines);
    let free = BlockContext {
        native_price: U256::ZERO,
        ..Default::default()
    };
    let lines = run("free_native", contracts, free, CALLEE, &[(PLENTY, 1 << 40)]);
    check("free_native", &lines);
}

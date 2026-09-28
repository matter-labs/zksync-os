//! EVM call execution outcomes: success, revert, invalid opcode, and out-of-gas.

use crate::test_support::{call_tx, call_tx_with, new_tester};
use rig::alloy::primitives::{address, U256 as AlloyU256};
use rig::alloy::signers::local::PrivateKeySigner;
use rig::constants::{
    CALL_GAS_LIMIT, DEFAULT_BALANCE, DEFAULT_MAX_FEE, DEFAULT_PRIORITY_FEE, TEST_CHAIN_ID,
};
use rig::evm_bytecode::{self, BytecodeBuilder};
use rig::evm_interpreter::native_resource_constants::{
    COPY_BASE_NATIVE_COST, COPY_BYTE_NATIVE_COST, RETURNDATACOPY_NATIVE_COST,
};
use rig::ruint::aliases::U256;
use rig::{assert_tx_reverted, assert_tx_success, testing_signer, BlockContext};

#[test]
fn out_of_gas_mid_execution() {
    let loop_bytecode = evm_bytecode::infinite_loop();
    let contract = address!("0000000000000000000000000000000000000101");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &loop_bytecode);

    let tx = call_tx(signer, contract, 25_000);
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);
}

#[test]
fn explicit_revert_no_data() {
    let revert_bytecode = evm_bytecode::revert();
    let contract = address!("0000000000000000000000000000000000000201");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &revert_bytecode);

    let tx = call_tx(signer, contract, CALL_GAS_LIMIT);
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);
}

#[test]
fn explicit_revert_with_data() {
    let revert_with_data = evm_bytecode::revert_with_data(&[0xde, 0xad, 0xbe, 0xef]);
    let contract = address!("0000000000000000000000000000000000000202");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &revert_with_data);

    let tx = call_tx(signer, contract, CALL_GAS_LIMIT);
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);

    let tx_out = output.tx_results[0].as_ref().unwrap();
    match &tx_out.execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Revert(data) => {
            assert_eq!(data, &[0xde, 0xad, 0xbe, 0xef]);
        }
        _ => panic!("expected revert with payload"),
    }
}

#[test]
fn invalid_opcode() {
    let invalid_bytecode = evm_bytecode::invalid_opcode();
    let contract = address!("0000000000000000000000000000000000000203");

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(contract, &invalid_bytecode);

    let tx = call_tx(signer, contract, CALL_GAS_LIMIT);
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);
}

#[test]
fn call_to_eoa_with_calldata_succeeds() {
    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let eoa = address!("0000000000000000000000000000000000000204");

    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));

    let tx = call_tx_with(
        signer,
        eoa,
        0,
        CALL_GAS_LIMIT,
        AlloyU256::ZERO,
        vec![0xca, 0xfe, 0xba, 0xbe],
        DEFAULT_MAX_FEE,
        DEFAULT_PRIORITY_FEE,
        TEST_CHAIN_ID,
    );
    let output = tester.execute_block(vec![tx]);
    assert_tx_success!(output, 0);
}

#[test]
fn nested_call_inner_reverts_outer_succeeds() {
    let inner_revert = evm_bytecode::revert();
    let inner_addr = address!("0000000000000000000000000000000000000205");

    let outer_addr = address!("0000000000000000000000000000000000000206");
    let outer_bytecode = BytecodeBuilder::new()
        .call_simple(inner_addr)
        .pop()
        .return_empty()
        .finish();

    let signer = PrivateKeySigner::random();
    let sender = signer.address();

    let mut tester = new_tester()
        .with_balance(sender, U256::from(DEFAULT_BALANCE))
        .with_evm_contract(inner_addr, &inner_revert)
        .with_evm_contract(outer_addr, &outer_bytecode);

    let tx = call_tx(signer, outer_addr, 200_000);
    let output = tester.execute_block(vec![tx]);
    assert_tx_success!(output, 0);
}

#[test]
fn oversized_sha3_lengths_charge_native_consistently() {
    let inner_addr = address!("0000000000000000000000000000000000000207");
    let outer_addr = address!("0000000000000000000000000000000000000208");
    let signer = testing_signer(0);

    let native_used = |len: U256| {
        let mut inner_bytecode = BytecodeBuilder::new()
            .push_bytes(&len.to_be_bytes::<32>())
            .push0()
            .finish();
        inner_bytecode.push(0x20); // SHA3
        let outer_bytecode = BytecodeBuilder::new()
            .call_simple(inner_addr)
            .pop()
            .return_empty()
            .finish();

        let mut tester = new_tester()
            .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
            .with_evm_contract(inner_addr, &inner_bytecode)
            .with_evm_contract(outer_addr, &outer_bytecode)
            // Make the refund native-bound so the optional RISC-V run also
            // detects a forward/proving state-diff mismatch.
            .with_block_context(BlockContext {
                native_price: U256::from(1_000u64),
                ..Default::default()
            });

        let output = tester.execute_block(vec![call_tx(signer.clone(), outer_addr, 2_000_000)]);
        assert_tx_success!(output, 0);
        output.tx_results[0]
            .as_ref()
            .expect("transaction should be processed")
            .computational_native_used
    };

    // 2^32 fits a 64-bit usize but not a 32-bit usize; 2^64 fits neither.
    assert_eq!(
        native_used(U256::from(1u64) << 32),
        native_used(U256::from(1u64) << 64),
        "SHA3 must charge its native cost before rejecting an oversized length"
    );
}

/// Pins that RETURNDATACOPY charges its length-dependent gas and native cost
/// from the 256-bit length, before the length is narrowed to `usize`. A charge
/// that follows the narrowing differs between the 64-bit forward host and the
/// 32-bit proving target, because only the forward host accepts a length of
/// 2^32. The rig then reports a forward/proving storage-diff mismatch, which
/// it checks when `ZKSYNC_RISC_V_RUN=true` or `CI` is set.
#[test]
fn oversized_returndatacopy_charges_native_consistently() {
    // 2^32 fits a 64-bit usize but not a 32-bit usize.
    const COPY_LEN: u64 = 1 << 32;
    // Native cost that RETURNDATACOPY must charge for `COPY_LEN` bytes before
    // it rejects the length.
    const COPY_NATIVE_COST: u64 =
        COPY_BYTE_NATIVE_COST * COPY_LEN + COPY_BASE_NATIVE_COST + RETURNDATACOPY_NATIVE_COST;
    // Gas allowance of the inner frame. It covers the ~403M gas copy charge.
    const INNER_CALL_GAS: u64 = 500_000_000;
    // Transaction gas limit. It admits the copy charge and still leaves enough
    // unused gas for the refund `delta_gas` adjustment to raise `gas_used`.
    const HIGH_TX_GAS_LIMIT: u64 = 5_000_000_000;
    // Ordinary transaction gas limit, far below the copy charge.
    const REALISTIC_TX_GAS_LIMIT: u64 = 2_000_000;

    let inner_addr = address!("0000000000000000000000000000000000000209");
    let outer_addr = address!("000000000000000000000000000000000000020a");
    let signer = testing_signer(0);

    // The inner contract copies 2^32 bytes while the returndata is empty, so
    // it fails the returndata bounds check and allocates no memory.
    let mut inner_bytecode = BytecodeBuilder::new()
        .push_bytes(&U256::from(COPY_LEN).to_be_bytes::<32>())
        .push0()
        .push0()
        .finish();
    inner_bytecode.push(0x3e); // RETURNDATACOPY

    // The outer contract calls the inner one and swallows the failure, so the
    // transaction succeeds. The call passes an explicit gas allowance, which
    // leaves the rest of the transaction gas unused.
    // CALL operands: out_size=0, out_offset=0, in_size=0, in_offset=0,
    // value=0, callee, gas.
    let outer_bytecode = BytecodeBuilder::new()
        .push0_n(5)
        .push_address(inner_addr)
        .push_bytes(&INNER_CALL_GAS.to_be_bytes())
        .call()
        .pop()
        .return_empty()
        .finish();

    let native_used = |tx_gas_limit: u64| {
        let mut tester = new_tester()
            .with_max_tx_gas_limit(HIGH_TX_GAS_LIMIT)
            .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
            .with_evm_contract(inner_addr, &inner_bytecode)
            .with_evm_contract(outer_addr, &outer_bytecode)
            // Price native so that the refund `delta_gas` adjustment carries
            // the native spend into committed fee state. The optional RISC-V
            // run then detects a forward/proving state-diff mismatch.
            .with_block_context(BlockContext {
                native_price: U256::from(200u64),
                ..Default::default()
            });

        let output = tester.execute_block(vec![call_tx(signer.clone(), outer_addr, tx_gas_limit)]);
        assert_tx_success!(output, 0);
        output.tx_results[0]
            .as_ref()
            .expect("transaction should be processed")
            .computational_native_used
    };

    assert!(
        native_used(HIGH_TX_GAS_LIMIT) >= COPY_NATIVE_COST,
        "RETURNDATACOPY must charge the copy native cost for an oversized length"
    );
    // Under a realistic gas limit the inner frame cannot pay the copy gas, so
    // the copy native cost stays uncharged.
    assert!(
        native_used(REALISTIC_TX_GAS_LIMIT) < COPY_NATIVE_COST,
        "the copy charge must remain gated by the available gas"
    );
}

/// Pins that a copy length whose byte-linear native cost fits in a `u64`, but
/// overflows once the opcode's base native cost is added, fails the copy with
/// out-of-gas instead of overflowing. Builds with overflow checks (such as the
/// sequencer) used to panic here, while builds without them (such as the
/// proving binary) wrapped the sum and still failed on the gas charge.
#[test]
fn copy_native_cost_overflow_is_out_of_gas() {
    // `COPY_BYTE_NATIVE_COST * COPY_LEN + COPY_BASE_NATIVE_COST` is
    // `u64::MAX - 1`, so adding any opcode base native cost overflows.
    const COPY_LEN: u64 = (u64::MAX - COPY_BASE_NATIVE_COST) / COPY_BYTE_NATIVE_COST;
    const _: () = assert!(
        COPY_BYTE_NATIVE_COST * COPY_LEN + COPY_BASE_NATIVE_COST
            > u64::MAX - RETURNDATACOPY_NATIVE_COST
    );

    const CALLDATACOPY: u8 = 0x37;
    const CODECOPY: u8 = 0x39;
    const RETURNDATACOPY: u8 = 0x3e;
    const MCOPY: u8 = 0x5e;
    let opcodes = [CALLDATACOPY, CODECOPY, RETURNDATACOPY, MCOPY];

    let signer = testing_signer(0);
    let mut tester = new_tester().with_balance(signer.address(), U256::from(DEFAULT_BALANCE));
    let mut txs = Vec::new();
    for (nonce, opcode) in opcodes.into_iter().enumerate() {
        let contract = rig::alloy::primitives::Address::with_last_byte(0x30 + nonce as u8);
        // Operands: len=COPY_LEN, then two zero offsets.
        let mut bytecode = BytecodeBuilder::new()
            .push_bytes(&COPY_LEN.to_be_bytes())
            .push0()
            .push0()
            .finish();
        bytecode.push(opcode);
        tester = tester.with_evm_contract(contract, &bytecode);
        txs.push(call_tx_with(
            signer.clone(),
            contract,
            nonce as u64,
            CALL_GAS_LIMIT,
            AlloyU256::ZERO,
            vec![],
            DEFAULT_MAX_FEE,
            DEFAULT_PRIORITY_FEE,
            TEST_CHAIN_ID,
        ));
    }

    let output = tester.execute_block(txs);
    for (i, opcode) in opcodes.into_iter().enumerate() {
        assert_tx_reverted!(output, i);
        assert_eq!(
            output.tx_results[i].as_ref().unwrap().gas_used,
            CALL_GAS_LIMIT,
            "copy opcode {opcode:#04x} must fail out of gas and consume the full gas limit"
        );
    }
}

/// Regression: running out of native while copying a callee's returndata into
/// the caller's heap (`copy_returndata_to_heap`, which charges native but no
/// gas) must revert the transaction like any other out-of-native, rather than
/// surfacing as an internal error that fails the whole block.
#[test]
fn out_of_native_during_returndata_copy_reverts_tx() {
    // Returndata copied into the caller's heap on every call.
    const RET_LEN: u64 = 8192;
    // Low native-per-gas ratio (1000 / 100 = 10), so native runs out before gas.
    const NATIVE_PRICE: u64 = 100;
    // Each loop iteration costs a few thousand gas. Stepping the gas limit
    // across more than one iteration moves the out-of-native point through
    // the whole iteration, so some of the transactions run out inside the
    // returndata copy, which dominates the per-iteration native cost.
    const BASE_GAS_LIMIT: u64 = 1_000_000;
    const GAS_LIMIT_STEP: u64 = 500;
    const NUM_TXS: u64 = 10;

    let returner_addr = address!("000000000000000000000000000000000000020b");
    let caller_addr = address!("000000000000000000000000000000000000020c");

    // RETURN(0, RET_LEN)
    let returner_bytecode = BytecodeBuilder::new()
        .push_bytes(&RET_LEN.to_be_bytes())
        .push0()
        .return_()
        .finish();

    // Loops forever: STATICCALL(gas, returner, 0, 0, 0, RET_LEN); POP.
    // STATICCALL operands: ret_size=RET_LEN, ret_offset=0, in_size=0,
    // in_offset=0, callee, gas.
    let mut caller_bytecode = BytecodeBuilder::new()
        .jumpdest()
        .push_bytes(&RET_LEN.to_be_bytes())
        .push0_n(3)
        .push_address(returner_addr)
        .gas()
        .finish();
    caller_bytecode.push(0xfa); // STATICCALL
    caller_bytecode.extend(BytecodeBuilder::new().pop().push0().jump().finish());

    let signer = testing_signer(0);
    let mut tester = new_tester()
        .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
        .with_evm_contract(returner_addr, &returner_bytecode)
        .with_evm_contract(caller_addr, &caller_bytecode)
        .with_block_context(BlockContext {
            native_price: U256::from(NATIVE_PRICE),
            ..Default::default()
        });

    let txs = (0..NUM_TXS)
        .map(|nonce| {
            call_tx_with(
                signer.clone(),
                caller_addr,
                nonce,
                BASE_GAS_LIMIT + nonce * GAS_LIMIT_STEP,
                AlloyU256::ZERO,
                vec![],
                DEFAULT_MAX_FEE,
                DEFAULT_PRIORITY_FEE,
                TEST_CHAIN_ID,
            )
        })
        .collect();
    let output = tester.execute_block(txs);

    for (i, tx_result) in output.tx_results.iter().enumerate() {
        assert_tx_reverted!(output, i);
        let tx_out = tx_result.as_ref().unwrap();
        assert_eq!(
            tx_out.gas_used,
            BASE_GAS_LIMIT + i as u64 * GAS_LIMIT_STEP,
            "out-of-native tx {i} must consume its full gas limit"
        );
    }
}

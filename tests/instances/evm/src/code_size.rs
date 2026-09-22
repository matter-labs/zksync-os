use crate::test_support::{call_tx_with, create_tx};
use rig::alloy::primitives::{address, Address};
use rig::constants::*;
use rig::evm_bytecode::BytecodeBuilder;
use rig::evm_interpreter::opcodes;
use rig::ruint::aliases::U256;
use rig::zk_ee::system::metadata::chain_config::{
    DEFAULT_MAX_CODE_SIZE, DEFAULT_MAX_TX_GAS_LIMIT, LARGE_MAX_CODE_SIZE,
};
use rig::zksync_os_interface::error::InvalidTransaction;
use rig::{assert_tx_reverted, assert_tx_success, TestingFramework};

const FACTORY: Address = address!("1000000000000000000000000000000000000000");

fn tester(large_contracts: bool) -> TestingFramework<false> {
    let tester = TestingFramework::new();
    let config = tester.chain_config().with_large_contracts(large_contracts);
    tester.with_chain_config(config)
}

fn constructor(runtime_size: u32) -> Vec<u8> {
    BytecodeBuilder::new()
        .push_u8(1)
        .push0()
        .sstore()
        .push_bytes(&runtime_size.to_be_bytes())
        .push0()
        .return_()
        .finish()
}

fn assert_code_and_storage(tester: &mut TestingFramework<false>, address: Address, size: u32) {
    let properties = tester.get_account_properties(&address);
    assert_eq!(properties.observable_bytecode_len, size);
    assert_eq!(properties.unpadded_code_len, size);
    assert_eq!(properties.nonce, 1);
    assert_eq!(
        tester
            .get_storage_slot(&address, U256::ZERO)
            .unwrap()
            .as_u8_array(),
        U256::ONE.to_be_bytes::<32>(),
    );
}

#[test]
fn runtime_code_size_boundaries() {
    for large in [false, true] {
        let limit = if large {
            LARGE_MAX_CODE_SIZE
        } else {
            DEFAULT_MAX_CODE_SIZE
        };
        for size in [limit - 1, limit, limit + 1] {
            let mut tester = tester(large);
            let signer = tester.prefunded_random_signer();
            let deployed = signer.address().create(0);
            let output = tester.execute_block(vec![create_tx(
                signer,
                DEFAULT_MAX_TX_GAS_LIMIT,
                constructor(size),
            )]);
            if size <= limit {
                assert_tx_success!(output, 0);
                assert_code_and_storage(&mut tester, deployed, size);
            } else {
                assert_tx_reverted!(output, 0);
                assert_eq!(
                    tester
                        .get_account_properties(&deployed)
                        .observable_bytecode_len,
                    0
                );
                assert_eq!(tester.get_account_properties(&deployed).nonce, 0);
                assert!(tester
                    .get_storage_slot(&deployed, U256::ZERO)
                    .is_none_or(|value| value.is_zero()));
            }
        }
    }
}

fn padded_constructor(initcode_size: usize) -> Vec<u8> {
    let mut tail = BytecodeBuilder::new().jumpdest().finish();
    tail.extend(constructor(1));
    let jump_offset = initcode_size - tail.len();
    let mut code = BytecodeBuilder::new()
        .push_bytes(&(jump_offset as u32).to_be_bytes())
        .jump()
        .finish();
    code.resize(jump_offset, opcodes::STOP);
    code.extend(tail);
    code
}

#[test]
fn transaction_initcode_size_boundaries() {
    for large in [false, true] {
        let limit = tester(large).chain_config().max_initcode_size() as usize;
        for size in [limit - 1, limit, limit + 1] {
            let mut tester = tester(large);
            let signer = tester.prefunded_random_signer();
            let sender = signer.address();
            let deployed = sender.create(0);
            // At the large limit this also executes a jump beyond the 16-bit PC range.
            let output = tester.execute_block(vec![create_tx(
                signer,
                DEFAULT_MAX_TX_GAS_LIMIT,
                padded_constructor(size),
            )]);
            if size <= limit {
                assert_tx_success!(output, 0);
                assert_code_and_storage(&mut tester, deployed, 1);
            } else {
                assert!(matches!(
                    output.tx_results[0],
                    Err(InvalidTransaction::CreateInitCodeSizeLimit)
                ));
                assert_eq!(tester.get_account_properties(&sender).nonce, 0);
                assert_eq!(tester.get_account_properties(&deployed).nonce, 0);
            }
        }
    }
}

fn factory(create2: bool) -> Vec<u8> {
    let mut code = BytecodeBuilder::new()
        .calldatasize()
        .push0()
        .push0()
        .finish();
    code.push(opcodes::CALLDATACOPY);
    if create2 {
        code.extend(BytecodeBuilder::new().push0().finish());
    }
    code.extend(
        BytecodeBuilder::new()
            .calldatasize()
            .push0()
            .push0()
            .finish(),
    );
    code.push(if create2 {
        opcodes::CREATE2
    } else {
        opcodes::CREATE
    });
    code.extend(BytecodeBuilder::new().push0().sstore().stop().finish());
    code
}

#[test]
fn create_and_create2_enforce_configured_limits() {
    for create2 in [false, true] {
        for large in [false, true] {
            let config = tester(large).chain_config();
            for (runtime_size, initcode_size, success, parent_reverts) in [
                (config.max_code_size(), None, true, false),
                (config.max_code_size() + 1, None, false, false),
                (1, Some(config.max_initcode_size() as usize), true, false),
                (
                    1,
                    Some(config.max_initcode_size() as usize + 1),
                    false,
                    true,
                ),
            ] {
                let initcode =
                    initcode_size.map_or_else(|| constructor(runtime_size), padded_constructor);
                let mut tester = tester(large).with_evm_contract(FACTORY, &factory(create2));
                let signer = tester.prefunded_random_signer();
                let output = tester.execute_block(vec![call_tx_with(
                    signer,
                    FACTORY,
                    0,
                    DEFAULT_MAX_TX_GAS_LIMIT,
                    U256::ZERO,
                    initcode,
                    DEFAULT_MAX_FEE,
                    DEFAULT_PRIORITY_FEE,
                    TEST_CHAIN_ID,
                )]);
                if parent_reverts {
                    assert_tx_reverted!(output, 0);
                } else {
                    assert_tx_success!(output, 0);
                }
                let address_word = tester
                    .get_storage_slot(&FACTORY, U256::ZERO)
                    .unwrap_or_default();
                if success {
                    assert!(!address_word.is_zero());
                    let deployed = Address::from_slice(&address_word.as_u8_array()[12..]);
                    assert_code_and_storage(&mut tester, deployed, runtime_size);
                } else {
                    assert!(address_word.is_zero());
                }
            }
        }
    }
}

#[test]
fn code_size_option_does_not_change_existing_deployment_gas() {
    let mut gas_used = Vec::new();
    for large in [false, true] {
        let mut tester = tester(large);
        let signer = tester.prefunded_random_signer();
        let output = tester.execute_block(vec![create_tx(
            signer,
            DEFAULT_MAX_TX_GAS_LIMIT,
            constructor(DEFAULT_MAX_CODE_SIZE),
        )]);
        assert_tx_success!(output, 0);
        gas_used.push(output.tx_results[0].as_ref().unwrap().gas_used);
    }
    assert_eq!(gas_used[0], gas_used[1]);
}

#[test]
fn large_contract_deployment_and_execution() {
    let runtime_tail = [
        opcodes::JUMPDEST,
        opcodes::CODESIZE,
        opcodes::PUSH0,
        opcodes::MSTORE,
        opcodes::PUSH1,
        32,
        opcodes::PUSH0,
        opcodes::RETURN,
    ];
    let runtime_jump = LARGE_MAX_CODE_SIZE as usize - runtime_tail.len();
    let mut runtime = BytecodeBuilder::new()
        .push_bytes(&(runtime_jump as u32).to_be_bytes())
        .jump()
        .finish();
    runtime.resize(runtime_jump, opcodes::STOP);
    runtime.extend(runtime_tail);

    // Copy from offset 65,536 in a 128 KiB initcode into a 64 KiB runtime.
    let mut initcode = BytecodeBuilder::new()
        .push_bytes(&LARGE_MAX_CODE_SIZE.to_be_bytes())
        .push_bytes(&LARGE_MAX_CODE_SIZE.to_be_bytes())
        .push0()
        .finish();
    initcode.push(opcodes::CODECOPY);
    initcode.extend(
        BytecodeBuilder::new()
            .push_bytes(&LARGE_MAX_CODE_SIZE.to_be_bytes())
            .push0()
            .return_()
            .finish(),
    );
    initcode.resize(LARGE_MAX_CODE_SIZE as usize, opcodes::STOP);
    initcode.extend(runtime);

    let mut tester = tester(true);
    let signer = tester.prefunded_random_signer();
    let deployed = signer.address().create(0);
    let output = tester.execute_block(vec![
        create_tx(signer.clone(), DEFAULT_MAX_TX_GAS_LIMIT, initcode),
        call_tx_with(
            signer.clone(),
            deployed,
            1,
            DEFAULT_GAS_LIMIT,
            U256::ZERO,
            vec![],
            DEFAULT_MAX_FEE,
            DEFAULT_PRIORITY_FEE,
            TEST_CHAIN_ID,
        ),
    ]);
    assert_tx_success!(output, 0);
    assert_tx_success!(output, 1);
    assert_eq!(
        tester
            .get_account_properties(&deployed)
            .observable_bytecode_len,
        LARGE_MAX_CODE_SIZE
    );
    use rig::zksync_os_interface::types::{ExecutionOutput, ExecutionResult};
    let ExecutionResult::Success(ExecutionOutput::Call(result)) =
        &output.tx_results[1].as_ref().unwrap().execution_result
    else {
        panic!("expected a successful call");
    };
    assert_eq!(
        result.as_slice(),
        U256::from(LARGE_MAX_CODE_SIZE).to_be_bytes::<32>()
    );

    tester.set_chain_config(tester.chain_config().with_large_contracts(false));
    let output = tester.execute_block(vec![call_tx_with(
        signer,
        deployed,
        2,
        DEFAULT_GAS_LIMIT,
        U256::ZERO,
        vec![],
        DEFAULT_MAX_FEE,
        DEFAULT_PRIORITY_FEE,
        TEST_CHAIN_ID,
    )]);
    assert_tx_success!(output, 0);
    assert_eq!(
        tester
            .get_account_properties(&deployed)
            .observable_bytecode_len,
        LARGE_MAX_CODE_SIZE
    );
}

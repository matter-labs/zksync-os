//! Contract deployment outcomes and constructor behavior.

use crate::test_support::{create_tx, new_tester};
use rig::alloy::primitives::address;
use rig::alloy::signers::local::PrivateKeySigner;
use rig::constants::{DEFAULT_BALANCE, DEPLOY_GAS_LIMIT};
use rig::evm_bytecode::{self, BytecodeBuilder};
use rig::ruint::aliases::U256;
use rig::{assert_tx_reverted, assert_tx_success};

#[test]
fn constructor_revert_fails_deployment() {
    let init_bytecode = evm_bytecode::revert();

    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));

    let tx = create_tx(signer, DEPLOY_GAS_LIMIT, init_bytecode);
    let output = tester.execute_block(vec![tx]);
    assert_tx_reverted!(output, 0);
}

#[test]
fn zero_length_deployed_code() {
    let init_bytecode = BytecodeBuilder::new().return_empty().finish();

    let signer = PrivateKeySigner::random();
    let sender = signer.address();
    let mut tester = new_tester().with_balance(sender, U256::from(DEFAULT_BALANCE));

    let tx = create_tx(signer, DEPLOY_GAS_LIMIT, init_bytecode);
    let output = tester.execute_block(vec![tx]);
    assert_eq!(output.tx_results.len(), 1);
    assert_tx_success!(output, 0);

    let tx_out = output.tx_results[0].as_ref().unwrap();
    match &tx_out.execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Success(
            rig::zksync_os_interface::types::ExecutionOutput::Create(data, address),
        ) => {
            assert!(data.is_empty(), "runtime code must be empty");
            assert_ne!(
                *address,
                address!("0000000000000000000000000000000000000000")
            );
        }
        _ => panic!("expected successful create execution output"),
    }
}

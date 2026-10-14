//! Call simulation must bound execution even when normal admission is skipped.

use rig::alloy::consensus::TxEip1559;
use rig::alloy::primitives::{address, TxKind};
use rig::constants::{DEFAULT_BALANCE, TEST_CHAIN_ID};
use rig::forward_system::run::simulate_tx;
use rig::ruint::aliases::U256;
use rig::utils::L1TxBuilder;
use rig::zk_ee::system::metadata::chain_config::DEFAULT_MAX_TX_GAS_LIMIT;
use rig::zk_ee::system::tracer::NopTracer;
use rig::zk_ee::system::validator::NopTxValidator;
use rig::zksync_os_interface::types::{ExecutionOutput, ExecutionResult, TxOutput};
use rig::zksync_os_tests_common::zksync_tx::encoding::ZKsyncOsEncodable;
use rig::zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;
use rig::{testing_signer, BlockContext, TestingFramework};

const TARGET: rig::alloy::primitives::Address =
    address!("0000000000000000000000000000000000010002");
// GAS; PUSH1 0; MSTORE; PUSH1 32; PUSH1 0; RETURN.
const RETURN_GAS: &[u8] = &[0x5a, 0x60, 0, 0x52, 0x60, 32, 0x60, 0, 0xf3];
// JUMPDEST; PUSH1 0; JUMP.
const INFINITE_LOOP: &[u8] = &[0x5b, 0x60, 0, 0x56];

fn call_tx(gas_limit: u64) -> ZKsyncTxEnvelope {
    ZKsyncTxEnvelope::from_eth_tx(
        TxEip1559 {
            chain_id: TEST_CHAIN_ID,
            gas_limit,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 1000,
            to: TxKind::Call(TARGET),
            ..Default::default()
        },
        testing_signer(0),
    )
}

fn new_tester(bytecode: &[u8], block_gas_limit: u64) -> TestingFramework<false> {
    TestingFramework::new()
        .with_balance(testing_signer(0).address(), U256::from(DEFAULT_BALANCE))
        .with_evm_contract(TARGET, bytecode)
        .with_block_context(BlockContext {
            gas_limit: block_gas_limit,
            // Isolate the EVM gas bound from native resource exhaustion.
            native_price: U256::ZERO,
            ..Default::default()
        })
}

fn simulate_call(tester: &TestingFramework<false>, tx: ZKsyncTxEnvelope) -> TxOutput {
    // Exercise the same single-call entrypoint as RPC, using the rig's state.
    // Call simulation is a forward-only facility; there is no proving run.
    let state = tester.prepare_batch_state();
    let input = tester.prepare_batch_block_input(vec![], tester.block_context().cloned());
    simulate_tx(
        tester.chain_config(),
        tx.encode(),
        input.block_context,
        state.tree,
        state.preimage_source,
        &mut NopTracer::default(),
        &mut NopTxValidator,
    )
    .expect("simulation must not fail internally")
    .expect("oversized gas must be clamped rather than rejected")
}

fn returned_gas(output: &TxOutput) -> U256 {
    let ExecutionResult::Success(ExecutionOutput::Call(data)) = &output.execution_result else {
        panic!("gas-observing call must succeed: {output:?}");
    };
    U256::from_be_slice(data)
}

#[test]
fn simulation_caps_l2_execution_gas_and_accounts_for_the_capped_budget() {
    for (block_limit, chain_limit) in [
        (100_000, DEFAULT_MAX_TX_GAS_LIMIT),
        (DEFAULT_MAX_TX_GAS_LIMIT + 1, DEFAULT_MAX_TX_GAS_LIMIT),
        (DEFAULT_MAX_TX_GAS_LIMIT + 2, DEFAULT_MAX_TX_GAS_LIMIT + 1),
    ] {
        let cap = block_limit.min(chain_limit);
        let tester = new_tester(RETURN_GAS, block_limit).with_max_tx_gas_limit(chain_limit);
        let capped = simulate_call(&tester, call_tx(cap));
        for gas_limit in [60_000, cap, cap + 1, 0xffffffffffff, u64::MAX] {
            let output = simulate_call(&tester, call_tx(gas_limit));
            // The first GAS opcode costs 2, after 21,000 intrinsic gas.
            assert_eq!(
                returned_gas(&output),
                U256::from(gas_limit.min(cap) - 21_002),
                "block_limit={block_limit}, chain_limit={chain_limit}, gas={gas_limit}"
            );
            assert_eq!(output.gas_used, capped.gas_used);
            assert_eq!(output.gas_refunded, capped.gas_refunded);
        }
    }
}

#[test]
fn simulation_exhausts_infinite_loop_at_the_gas_cap() {
    let cap = 100_000;
    let tester = new_tester(INFINITE_LOOP, cap);
    let output = simulate_call(&tester, call_tx(0xffffffffffff));
    assert!(!output.is_success());
    assert_eq!(output.gas_used, cap);
}

#[test]
fn simulation_caps_l1_execution_gas() {
    let cap = 1_000_000;
    let mut tester = new_tester(RETURN_GAS, cap);
    tester.mint_tokens_to_treasury();
    let tx = |gas_limit| {
        L1TxBuilder::new()
            .from(testing_signer(0).address())
            .to(TARGET)
            .gas_limit(gas_limit)
            .gas_price(1000)
            .build()
    };
    let capped = simulate_call(&tester, tx(u128::from(cap)));
    for gas_limit in [cap + 1, 0xffffffffffff, u64::MAX] {
        let output = simulate_call(&tester, tx(u128::from(gas_limit)));
        assert_eq!(returned_gas(&output), returned_gas(&capped));
        assert_eq!(output.gas_used, capped.gas_used);
        assert_eq!(output.gas_refunded, capped.gas_refunded);
    }
}

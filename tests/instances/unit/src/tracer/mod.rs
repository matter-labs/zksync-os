#![cfg(test)]

pub mod call_tracer;
pub mod evm_opcodes_logger;
pub mod tracer_event_hook;
pub mod tracer_storage_hooks;

use rig::alloy::consensus::TxEip2930;
use rig::alloy::primitives::{Address, TxKind, U256};
use rig::forward_system::system::tracers::call_tracer::CallTracer;
use rig::zk_ee::system::validator::NopTxValidator;
use rig::{BlockContext, TestingFramework};
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

pub(crate) fn run_chain_with_tracer(
    to: Address,
    contracts: Vec<(Address, Vec<u8>)>,
    tracer: &mut CallTracer,
    block_context: Option<BlockContext>,
) {
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();
    tester = tester.with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

    for (address, bytecode) in contracts {
        tester.set_evm_contract(address, &bytecode);
    }

    // Create transaction to call the contract
    let tx = {
        let tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 100_000,
            to: TxKind::Call(to),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone())
    };

    tester.set_block_context(block_context);
    let _ = tester.execute_block_with_tracing(vec![tx], tracer, &mut NopTxValidator::default());
}

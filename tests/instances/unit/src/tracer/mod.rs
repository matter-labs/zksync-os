#![cfg(test)]

pub mod call_tracer;
pub mod evm_opcodes_logger;
pub mod tracer_event_hook;
pub mod tracer_storage_hooks;

use rig::alloy::consensus::TxEip2930;
use rig::alloy::primitives::{Address, TxKind, U256};
use rig::forward_system::run::convert_alloy::FromAlloy;
use rig::forward_system::system::tracers::call_tracer::CallTracer;
use rig::ruint::aliases::B160;
use rig::zk_ee::system::validator::NopTxValidator;
use rig::{BlockContext, Chain};
use zksync_os_tests_common::zksync_tx::encoding::ZKsyncOsEncodable;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

pub(crate) fn run_chain_with_tracer(
    to: Address,
    contracts: Vec<(Address, Vec<u8>)>,
    tracer: &mut CallTracer,
    block_context: Option<BlockContext>,
) {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();

    chain.set_balance(
        B160::from_alloy(wallet.address()),
        U256::from(1_000_000_000_000_000_u64),
    );

    for (address, bytecode) in contracts {
        chain.set_evm_bytecode(B160::from_alloy(address), &bytecode);
    }

    // Create transaction to call the contract
    let encoded_tx = {
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
        ZKsyncTxEnvelope::from_eth_tx(tx, wallet.clone()).encode()
    };

    let _ = chain.run_block_with_extra_stats(
        vec![encoded_tx],
        block_context,
        None,
        None,
        tracer,
        &mut NopTxValidator::default(),
    );
}

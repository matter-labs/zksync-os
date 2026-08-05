#![cfg(test)]

pub mod call_tracer;
pub mod evm_opcodes_logger;
pub mod tracer_event_hook;
pub mod tracer_storage_hooks;

use rig::alloy::consensus::TxEip2930;
use rig::alloy::primitives::{Address, TxKind, U256};
use rig::forward_system::system::tracers::call_tracer::CallTracer;
use rig::ruint::aliases::B160;
use rig::Chain;

pub(crate) fn run_chain_with_tracer(
    to: Address,
    contracts: Vec<(Address, Vec<u8>)>,
    tracer: &mut CallTracer,
) {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();

    chain.set_balance(
        B160::from_be_bytes(wallet.address().into_array()),
        U256::from(1_000_000_000_000_000_u64),
    );

    for (address, bytecode) in contracts {
        chain.set_evm_bytecode(B160::from_be_bytes(address.into_array()), &bytecode);
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
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    let result = chain.run_block_with_extra_stats(vec![encoded_tx], None, None, None, tracer);

    assert!(result.is_ok(), "Block execution should succeed");
    let (block_output, _, _) = result.unwrap();
    assert!(
        block_output.tx_results[0].is_ok(),
        "Transaction should succeed. Result: {:?}",
        block_output.tx_results[0]
    );
}

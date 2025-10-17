use forward_system::run::output::BlockOutput;
use rig::BlockContext;
use rig::{
    alloy::consensus::TxLegacy,
    alloy::primitives::{address, Address},
    utils::{calldata_for_forwarder, FORWARDER_BYTECODE},
};
use ruint::aliases::{B160, U256};

// Creates two txs:
/// 1. Calls the precompile with given input and gas limit.
/// 2. Calls the forwarder contract to call the precompile with the same input and gas limit.
///
/// The second call is just there to check consistency between forward and proof runs.
pub fn run_precompile(id: &str, input: &[u8]) -> BlockOutput {
    let gas = 1 << 27;
    let mut chain = rig::Chain::empty(None);
    let wallet = chain.random_signer();
    let target = Address::from_slice(hex::decode(id).unwrap().as_slice());
    let forwarder = address!("0x1000000000000000000000000000000000000000");

    chain.set_balance(
        B160::from_be_bytes(wallet.address().into_array()),
        U256::from(1_000_000_000_000_000_u64),
    );
    chain.set_evm_bytecode(
        B160::from_be_bytes(forwarder.into_array()),
        &hex::decode(FORWARDER_BYTECODE).unwrap(),
    );

    let direct_tx = rig::utils::sign_and_encode_alloy_tx(
        TxLegacy {
            chain_id: 37u64.into(),
            nonce: 0,
            gas_price: 25_000,
            gas_limit: gas,
            to: rig::alloy::primitives::TxKind::Call(target),
            value: Default::default(),
            input: input.to_vec().into(),
        },
        &wallet,
    );

    let calldata = calldata_for_forwarder(target, input);
    let forwarded_tx = rig::utils::sign_and_encode_alloy_tx(
        TxLegacy {
            chain_id: 37u64.into(),
            nonce: 1,
            gas_price: 25_000,
            gas_limit: gas,
            to: rig::alloy::primitives::TxKind::Call(forwarder),
            value: Default::default(),
            input: calldata.into(),
        },
        &wallet,
    );
    // We use a very high native per gas ratio
    let block_context = BlockContext {
        native_price: U256::ONE,
        eip1559_basefee: U256::from(25_000),
        ..Default::default()
    };

    let run_config = rig::chain::RunConfig {
        app: Some("for_tests".to_string()),
        only_forward: false,
        check_storage_diff_hashes: true,
        ..Default::default()
    };
    chain.run_block(
        vec![direct_tx, forwarded_tx],
        Some(block_context),
        Some(run_config),
    )
}

#[allow(dead_code)]
pub fn is_zero(data: &[u8]) -> bool {
    data.iter().all(|b| *b == 0)
}

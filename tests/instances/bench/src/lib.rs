#![cfg(test)]
use rig::{
    alloy::{self, primitives::TxKind, rpc::types::TransactionRequest},
    ruint::aliases::U256,
    TestingFramework,
};
use std::path::PathBuf;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

// WASM disabled for now
// #[test]
// #[ignore]
// fn memory_alloc_heavy() {
//     let mut chain = rig::Chain::empty(None);
//     let wallet = chain.random_wallet();
//
//     let c_addr = Address::from_low_u64_ne(1);
//     let c_bytes = rig::utils::load_wasm_bytecode("bench");
//     chain
//         .set_wasm_bytecode(B160::from_be_bytes(c_addr.0), &c_bytes)
//         .set_balance(
//             B160::from_be_bytes(wallet.address().0),
//             U256::from(1_000_000_000_000_000_u64),
//         );
//
//     let tx = rig::utils::tx_encoding::sign_and_encode_ethers_legacy_tx(
//         TransactionRequest::new()
//             .to(c_addr)
//             .gas(10_000_000)
//             .gas_price(1000)
//             .data(rig::utils::construct_calldata(
//                 "681aa816",
//                 &["0000000000000000000000000000000000000000000000000000000000000001"],
//             ))
//             .nonce(0),
//         &wallet,
//     );
//
//     let mut pc = rig::ProfilerConfig::new(PathBuf::from(format!(
//         "{}/os_profile.svg",
//         env!("CARGO_MANIFEST_DIR")
//     )));
//     pc.frequency_recip = 1;
//     chain.run_block(vec![tx], None, Some(pc));
// }

// WASM disabled for now
// #[test]
// #[ignore = "IWASM integer acceleration ops are invalid in the implementation"]
// fn fibish_wasm() {
//     let mut chain = rig::Chain::empty(None);
//     let wallet = chain.random_wallet();
//
//     let c_addr = Address::from_low_u64_ne(1);
//     let c_bytes = rig::utils::load_wasm_bytecode("bench");
//     chain
//         .set_wasm_bytecode(B160::from_be_bytes(c_addr.0), &c_bytes)
//         .set_balance(
//             B160::from_be_bytes(wallet.address().0),
//             U256::from(1_000_000_000_000_000_u64),
//         );
//
//     let tx = rig::utils::tx_encoding::sign_and_encode_ethers_legacy_tx(
//         TransactionRequest::new()
//             .to(c_addr)
//             .gas(1 << 27)
//             .gas_price(1000)
//             .data(rig::utils::construct_calldata(
//                 "0x70e31497",
//                 &[
//                     "0000000000000000000000000000000000000000000000000000000000000001",
//                     "0000000000000000000000000000000000000000000000000000000000000003",
//                     "0000000000000000000000000000000000000000000000000000000000000002",
//                 ],
//             ))
//             .nonce(0),
//         &wallet,
//     );
//
//     let mut pc = rig::ProfilerConfig::new(PathBuf::from(format!(
//         "{}/os_profile_fibish_wasm.svg",
//         env!("CARGO_MANIFEST_DIR")
//     )));
//     pc.frequency_recip = 1;
//     chain.run_block(vec![tx], None, Some(pc));
// }

#[test]
fn fibish_sol() {
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();

    let c_addr = alloy::primitives::Address::from(alloy::primitives::U160::from(1));
    let c_bytes = rig::utils::load_sol_bytecode("bench", "arith");
    tester = tester
        .with_evm_contract(c_addr, &c_bytes)
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

    let tx = TransactionRequest {
        to: Some(TxKind::Call(c_addr)),
        gas: Some(1 << 27),
        gas_price: Some(1000),
        input: rig::utils::construct_calldata(
            "0x9714e370",
            &[
                "0000000000000000000000000000000000000000000000000000000000000001",
                "0000000000000000000000000000000000000000000000000000000000000003",
                "0000000000000000000000000000000000000000000000000000000000000002",
            ],
        )
        .into(),
        nonce: Some(0),
        ..Default::default()
    };

    let tx = ZKsyncTxEnvelope::from_eth_tx_from_req(tx, wallet);

    let mut pc = rig::ProfilerConfig::new(PathBuf::from(format!(
        "{}/os_profile_fibish_sol.svg",
        env!("CARGO_MANIFEST_DIR")
    )));
    pc.frequency_recip = 1;
    let run_config = rig::chain::RunConfig {
        profiler_config: Some(pc),
        ..Default::default()
    };
    tester = tester.with_run_config(run_config);
    tester.execute_block(vec![tx]);
}

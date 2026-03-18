#![cfg(test)]
use rig::{
    alloy::{
        primitives::{address, TxKind},
        rpc::types::TransactionRequest,
    },
    ruint::aliases::U256,
    TestingFramework,
};
use std::path::PathBuf;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

#[test]
fn get_name_sol() {
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();

    let erc20_addr = address!("0000000000000000000000000000000000000001");
    let erc20_bytecode = rig::utils::load_sol_bytecode("erc20", "erc20");
    tester = tester
        .with_evm_contract(erc20_addr, &erc20_bytecode)
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

    let tx_get_name = ZKsyncTxEnvelope::from_eth_tx_from_req(
        TransactionRequest {
            to: Some(TxKind::Call(erc20_addr)),
            gas: Some(1 << 27),
            gas_price: Some(1000),
            input: rig::utils::construct_calldata("0x06fdde03", &[]).into(),
            nonce: Some(0),
            ..Default::default()
        },
        wallet,
    );

    let run_config = rig::chain::RunConfig {
        flamegraph_output: Some(PathBuf::from(format!(
            "{}/os_profile_get_name_sol.svg",
            env!("CARGO_MANIFEST_DIR")
        ))),
        ..Default::default()
    };
    tester = tester.with_run_config(run_config);
    tester.execute_block(vec![tx_get_name]);
}

// WASM disabled for now
// #[test]
// fn get_name_wasm() {
//     let mut chain = rig::Chain::empty(None);
//     let wallet = chain.random_wallet();
//
//     let erc20_addr = Address::from_low_u64_ne(1);
//     let erc20_bytecode = rig::utils::load_wasm_bytecode("c_erc20");
//     chain
//         .set_wasm_bytecode(B160::from_be_bytes(erc20_addr.0), &erc20_bytecode)
//         .set_balance(
//             B160::from_be_bytes(wallet.address().0),
//             U256::from(1_000_000_000_000_000_u64),
//         );
//
//     let tx_get_name = rig::utils::tx_encoding::sign_and_encode_ethers_legacy_tx(
//         TransactionRequest::new()
//             .to(erc20_addr)
//             .gas(1 << 27)
//             .gas_price(1000)
//             .data(rig::utils::construct_calldata("0x03defd06", &[]))
//             .nonce(0),
//         &wallet,
//     );
//
//     let mut pc = rig::ProfilerConfig::new(PathBuf::from(format!(
//         "{}/os_profile_get_name_wasm.svg",
//         env!("CARGO_MANIFEST_DIR")
//     )));
//     pc.frequency_recip = 1;
//     chain.run_block(vec![tx_get_name], None, Some(pc));
// }

#[test]
fn balance_of_sol() {
    let mut tester = TestingFramework::new();
    let wallet = tester.random_signer();

    let erc20_addr = address!("0000000000000000000000000000000000000001");
    let erc20_bytecode = rig::utils::load_sol_bytecode("erc20", "erc20");
    tester = tester
        .with_evm_contract(erc20_addr, &erc20_bytecode)
        .with_balance(wallet.address(), U256::from(1_000_000_000_000_000_u64));

    let tx_mint = ZKsyncTxEnvelope::from_eth_tx_from_req(
        TransactionRequest {
            to: Some(TxKind::Call(erc20_addr)),
            gas: Some(1u64 << 27),
            gas_price: Some(1000),
            input: rig::utils::construct_calldata(
                "0x40c10f19",
                &[
                    &format!("{:x}", wallet.address()),
                    "0000000000000000000000000000000000000000000000000000000000001000",
                ],
            )
            .into(),
            nonce: Some(0),
            ..Default::default()
        },
        wallet.clone(),
    );

    let tx_balance = ZKsyncTxEnvelope::from_eth_tx_from_req(
        TransactionRequest {
            to: Some(TxKind::Call(erc20_addr)),
            gas: Some(1u64 << 27),
            gas_price: Some(1000),
            input: rig::utils::construct_calldata(
                "0x70a08231",
                &[&format!("{:x}", wallet.address())],
            )
            .into(),
            nonce: Some(1),
            ..Default::default()
        },
        wallet,
    );

    let run_config = rig::chain::RunConfig {
        flamegraph_output: Some(PathBuf::from(format!(
            "{}/os_profile_balance_of_sol.svg",
            env!("CARGO_MANIFEST_DIR")
        ))),
        ..Default::default()
    };
    tester = tester.with_run_config(run_config);
    tester.execute_block(vec![tx_mint, tx_balance]);
}

// WASM disabled for now
// #[ignore = "Triggers a memory overlap in WASM"]
// #[test]
// fn balance_of_wasm() {
//     let mut chain = rig::Chain::empty(None);
//     let wallet = chain.random_wallet();
//
//     let erc20_addr = Address::from_low_u64_ne(1);
//     let erc20_bytecode = rig::utils::load_wasm_bytecode("c_erc20");
//     chain
//         .set_wasm_bytecode(B160::from_be_bytes(erc20_addr.0), &erc20_bytecode)
//         .set_balance(
//             B160::from_be_bytes(wallet.address().0),
//             U256::from(1_000_000_000_000_000_u64),
//         );
//
//     let tx_mint = rig::utils::tx_encoding::sign_and_encode_ethers_legacy_tx(
//         TransactionRequest::new()
//             .to(erc20_addr)
//             .gas(1u64 << 27)
//             .gas_price(1000)
//             .data(rig::utils::construct_calldata(
//                 "0x10f1046b",
//                 &[
//                     &format!("{:x}", wallet.address()),
//                     "0000000000000000000000000000000000000000000000000000000000001234",
//                 ],
//             ))
//             .nonce(0),
//         &wallet,
//     );
//
//     let tx_balance = rig::utils::tx_encoding::sign_and_encode_ethers_legacy_tx(
//         TransactionRequest::new()
//             .to(erc20_addr)
//             .gas(1u64 << 27)
//             .gas_price(1000)
//             .data(rig::utils::construct_calldata(
//                 "0x3182a070",
//                 &[&format!("{:x}", wallet.address())],
//             ))
//             .nonce(1),
//         &wallet,
//     );
//
//     let mut pc = rig::ProfilerConfig::new(PathBuf::from(format!(
//         "{}/os_profile_balance_of_wasm.svg",
//         env!("CARGO_MANIFEST_DIR")
//     )));
//     pc.frequency_recip = 1;
//     chain.run_block(vec![tx_mint, tx_balance], None, Some(pc));
// }

#[test]
fn transfer_sol() {
    let mut tester = TestingFramework::new();
    let wallet_a = tester.random_signer();
    let wallet_b = tester.random_signer();

    let erc20_addr = address!("0000000000000000000000000000000000000001");
    let erc20_bytecode = rig::utils::load_sol_bytecode("erc20", "erc20");
    tester = tester
        .with_evm_contract(erc20_addr, &erc20_bytecode)
        .with_balance(wallet_a.address(), U256::from(1_000_000_000_000_000_u64));

    let tx_mint = ZKsyncTxEnvelope::from_eth_tx_from_req(
        TransactionRequest {
            to: Some(TxKind::Call(erc20_addr)),
            gas: Some(1u64 << 27),
            gas_price: Some(1000),
            input: rig::utils::construct_calldata(
                "0x40c10f19",
                &[
                    &format!("{:x}", wallet_a.address()),
                    "0000000000000000000000000000000000000000000000000000000000001000",
                ],
            )
            .into(),
            nonce: Some(0),
            ..Default::default()
        },
        wallet_a.clone(),
    );

    let tx_transfer = ZKsyncTxEnvelope::from_eth_tx_from_req(
        TransactionRequest {
            to: Some(TxKind::Call(erc20_addr)),
            gas: Some(1u64 << 27),
            gas_price: Some(1000),
            input: rig::utils::construct_calldata(
                "0xa9059cbb",
                &[
                    &format!("{:x}", wallet_b.address()),
                    "0000000000000000000000000000000000000000000000000000000000000100",
                ],
            )
            .into(),
            nonce: Some(1),
            ..Default::default()
        },
        wallet_a,
    );

    let run_config = rig::chain::RunConfig {
        flamegraph_output: Some(PathBuf::from(format!(
            "{}/os_profile_transfer_sol.svg",
            env!("CARGO_MANIFEST_DIR")
        ))),
        ..Default::default()
    };
    tester = tester.with_run_config(run_config);
    tester.execute_block(vec![tx_mint, tx_transfer]);
}

// WASM disabled for now
// #[ignore = "Triggers a memory overlap in WASM"]
// #[test]
// fn transfer_wasm() {
//     let mut chain = rig::Chain::empty(None);
//     let wallet_a = chain.random_wallet();
//     let wallet_b = chain.random_wallet();
//
//     let erc20_addr = Address::from_low_u64_ne(1);
//     let erc20_bytecode = rig::utils::load_wasm_bytecode("c_erc20");
//     chain
//         .set_wasm_bytecode(B160::from_be_bytes(erc20_addr.0), &erc20_bytecode)
//         .set_balance(
//             B160::from_be_bytes(wallet_a.address().0),
//             U256::from(1_000_000_000_000_000_u64),
//         );
//
//     let tx_mint = rig::utils::tx_encoding::sign_and_encode_ethers_legacy_tx(
//         TransactionRequest::new()
//             .to(erc20_addr)
//             .gas(1u64 << 27)
//             .gas_price(1000)
//             .data(rig::utils::construct_calldata(
//                 "0x10f1046b",
//                 &[
//                     &format!("{:x}", wallet_a.address()),
//                     "0000000000000000000000000000000000000000000000000000000000001000",
//                 ],
//             ))
//             .nonce(0),
//         &wallet_a,
//     );
//
//     let tx_transfer = rig::utils::tx_encoding::sign_and_encode_ethers_legacy_tx(
//         TransactionRequest::new()
//             .to(erc20_addr)
//             .gas(1u64 << 27)
//             .gas_price(1000)
//             .data(rig::utils::construct_calldata(
//                 "0xbb9c05a9",
//                 &[
//                     &format!("{:x}", wallet_b.address()),
//                     "0000000000000000000000000000000000000000000000000000000000000001",
//                 ],
//             ))
//             .nonce(1),
//         &wallet_a,
//     );
//
//     let mut pc = rig::ProfilerConfig::new(PathBuf::from(format!(
//         "{}/os_profile_transfer_wasm.svg",
//         env!("CARGO_MANIFEST_DIR")
//     )));
//     pc.frequency_recip = 1;
//     chain.run_block(vec![tx_mint, tx_transfer], None, Some(pc));
// }

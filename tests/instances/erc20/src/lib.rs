#![cfg(test)]
use rig::{
    ethers::{abi::Address, signers::Signer, types::TransactionRequest},
    ruint::aliases::{B160, U256},
};
use std::path::PathBuf;

#[test]
fn get_name_sol() {
    let mut chain = rig::Chain::empty(None);
    let wallet = chain.random_wallet();

    let erc20_addr = Address::from_low_u64_ne(1);
    let erc20_bytecode = rig::utils::load_sol_bytecode("erc20", "erc20");
    chain
        .set_evm_bytecode(B160::from_be_bytes(erc20_addr.0), &erc20_bytecode)
        .set_balance(
            B160::from_be_bytes(wallet.address().0),
            U256::from(1_000_000_000_000_000_u64),
        );

    let tx_get_name = rig::utils::sign_and_encode_ethers_legacy_tx(
        TransactionRequest::new()
            .to(erc20_addr)
            .gas(1 << 27)
            .gas_price(1000)
            .data(rig::utils::construct_calldata("0x06fdde03", &[]))
            .nonce(0),
        &wallet,
    );

    let mut pc = rig::ProfilerConfig::new(PathBuf::from(format!(
        "{}/os_profile_get_name_sol.svg",
        env!("CARGO_MANIFEST_DIR")
    )));
    pc.frequency_recip = 1;
    let run_config = rig::chain::RunConfig {
        profiler_config: Some(pc),
        ..Default::default()
    };
    chain.run_block(vec![tx_get_name], None, Some(run_config));
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
//     let tx_get_name = rig::utils::sign_and_encode_ethers_legacy_tx(
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
    let mut chain = rig::Chain::empty(None);
    let wallet = chain.random_wallet();

    let erc20_addr = Address::from_low_u64_ne(1);
    let erc20_bytecode = rig::utils::load_sol_bytecode("erc20", "erc20");
    chain
        .set_evm_bytecode(B160::from_be_bytes(erc20_addr.0), &erc20_bytecode)
        .set_balance(
            B160::from_be_bytes(wallet.address().0),
            U256::from(1_000_000_000_000_000_u64),
        );

    let tx_mint = rig::utils::sign_and_encode_ethers_legacy_tx(
        TransactionRequest::new()
            .to(erc20_addr)
            .gas(1u64 << 27)
            .gas_price(1000)
            .data(rig::utils::construct_calldata(
                "0x40c10f19",
                &[
                    &format!("{:x}", wallet.address()),
                    "0000000000000000000000000000000000000000000000000000000000001000",
                ],
            ))
            .nonce(0),
        &wallet,
    );

    let tx_balance = rig::utils::sign_and_encode_ethers_legacy_tx(
        TransactionRequest::new()
            .to(erc20_addr)
            .gas(1u64 << 27)
            .gas_price(1000)
            .data(rig::utils::construct_calldata(
                "0x70a08231",
                &[&format!("{:x}", wallet.address())],
            ))
            .nonce(1),
        &wallet,
    );

    let mut pc = rig::ProfilerConfig::new(PathBuf::from(format!(
        "{}/os_profile_balance_of_sol.svg",
        env!("CARGO_MANIFEST_DIR")
    )));
    pc.frequency_recip = 1;
    let run_config = rig::chain::RunConfig {
        profiler_config: Some(pc),
        ..Default::default()
    };
    chain.run_block(vec![tx_mint, tx_balance], None, Some(run_config));
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
//     let tx_mint = rig::utils::sign_and_encode_ethers_legacy_tx(
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
//     let tx_balance = rig::utils::sign_and_encode_ethers_legacy_tx(
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
    let mut chain = rig::Chain::empty(None);
    let wallet_a = chain.random_wallet();
    let wallet_b = chain.random_wallet();

    let erc20_addr = Address::from_low_u64_ne(1);
    let erc20_bytecode = rig::utils::load_sol_bytecode("erc20", "erc20");
    chain
        .set_evm_bytecode(B160::from_be_bytes(erc20_addr.0), &erc20_bytecode)
        .set_balance(
            B160::from_be_bytes(wallet_a.address().0),
            U256::from(1_000_000_000_000_000_u64),
        );

    let tx_mint = rig::utils::sign_and_encode_ethers_legacy_tx(
        TransactionRequest::new()
            .to(erc20_addr)
            .gas(1u64 << 27)
            .gas_price(1000)
            .data(rig::utils::construct_calldata(
                "0x40c10f19",
                &[
                    &format!("{:x}", wallet_a.address()),
                    "0000000000000000000000000000000000000000000000000000000000001000",
                ],
            ))
            .nonce(0),
        &wallet_a,
    );

    let tx_transfer = rig::utils::sign_and_encode_ethers_legacy_tx(
        TransactionRequest::new()
            .to(erc20_addr)
            .gas(1u64 << 27)
            .gas_price(1000)
            .data(rig::utils::construct_calldata(
                "0xa9059cbb",
                &[
                    &format!("{:x}", wallet_b.address()),
                    "0000000000000000000000000000000000000000000000000000000000000100",
                ],
            ))
            .nonce(1),
        &wallet_a,
    );

    let mut pc = rig::ProfilerConfig::new(PathBuf::from(format!(
        "{}/os_profile_transfer_sol.svg",
        env!("CARGO_MANIFEST_DIR")
    )));
    pc.frequency_recip = 1;
    let run_config = rig::chain::RunConfig {
        profiler_config: Some(pc),
        ..Default::default()
    };
    chain.run_block(vec![tx_mint, tx_transfer], None, Some(run_config));
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
//     let tx_mint = rig::utils::sign_and_encode_ethers_legacy_tx(
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
//     let tx_transfer = rig::utils::sign_and_encode_ethers_legacy_tx(
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

#![cfg(test)]
use rig::{
    ethers::{abi::Address, signers::Signer, types::TransactionRequest},
    ruint::aliases::{B160, U256},
};
use std::path::PathBuf;

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
//     let tx = rig::utils::sign_and_encode_ethers_legacy_tx(
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
//     let tx = rig::utils::sign_and_encode_ethers_legacy_tx(
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
    let mut chain = rig::Chain::empty(None);
    let wallet = chain.random_wallet();

    let c_addr = Address::from_low_u64_ne(1);
    let c_bytes = rig::utils::load_sol_bytecode("bench", "arith");
    chain
        .set_evm_bytecode(B160::from_be_bytes(c_addr.0), &c_bytes)
        .set_balance(
            B160::from_be_bytes(wallet.address().0),
            U256::from(1_000_000_000_000_000_u64),
        );

    let tx = rig::utils::sign_and_encode_ethers_legacy_tx(
        TransactionRequest::new()
            .to(c_addr)
            .gas(1 << 27)
            .gas_price(1000)
            .data(rig::utils::construct_calldata(
                "0x9714e370",
                &[
                    "0000000000000000000000000000000000000000000000000000000000000001",
                    "0000000000000000000000000000000000000000000000000000000000000003",
                    "0000000000000000000000000000000000000000000000000000000000000002",
                ],
            ))
            .nonce(0),
        &wallet,
    );

    let mut pc = rig::ProfilerConfig::new(PathBuf::from(format!(
        "{}/os_profile_fibish_sol.svg",
        env!("CARGO_MANIFEST_DIR")
    )));
    pc.frequency_recip = 1;
    let run_config = rig::chain::RunConfig {
        profiler_config: Some(pc),
        ..Default::default()
    };
    chain.run_block(vec![tx], None, Some(run_config));
}

mod initial_slot_regression;

// WASM disabled for now
// #[cfg(test)]
// mod tests {
//     use rig::ethers::signers::Signer;
//     use rig::{
//         ethers::types::{Address, TransactionRequest},
//         ruint::aliases::{B160, U256},
//     };
//     use std::path::PathBuf;
//
//     #[test]
//     fn message_from() {
//         let mut chain = rig::Chain::empty(None);
//         let wallet = chain.random_wallet();
//
//         let c_addr = Address::from_low_u64_ne(1);
//         let c_bytes = rig::utils::load_wasm_bytecode("unit");
//         chain
//             .set_wasm_bytecode(B160::from_be_bytes(c_addr.0), &c_bytes)
//             .set_balance(
//                 B160::from_be_bytes(wallet.address().0),
//                 U256::from(1_000_000_000_000_000_u64),
//             );
//
//         let tx_get_name = rig::utils::sign_and_encode_ethers_legacy_tx(
//             TransactionRequest::new()
//                 .to(c_addr)
//                 .gas(1 << 27)
//                 .gas_price(1000)
//                 .data(rig::utils::construct_calldata("0x01000000", &[]))
//                 .nonce(0),
//             &wallet,
//         );
//
//         println!("::: Addr {:#?}", wallet.address());
//         println!("::: Byte {:0x?}", wallet.address().0);
//
//         let mut pc = rig::ProfilerConfig::new(PathBuf::from(format!(
//             "{}/os_profile_message_from.svg",
//             env!("CARGO_MANIFEST_DIR")
//         )));
//         pc.frequency_recip = 1;
//
//         let _r = chain.run_block(vec![tx_get_name], None, Some(pc));
//
//         // assert_eq!(&wallet.address().0, r.tx_results[0].as_ref().unwrap().as_returned_bytes());
//     }
//
//     #[test]
//     fn hash_keccak256() {
//         let mut chain = rig::Chain::empty(None);
//         let wallet = chain.random_wallet();
//
//         let c_addr = Address::from_low_u64_ne(1);
//         let c_bytes = rig::utils::load_wasm_bytecode("unit");
//         chain
//             .set_wasm_bytecode(B160::from_be_bytes(c_addr.0), &c_bytes)
//             .set_balance(
//                 B160::from_be_bytes(wallet.address().0),
//                 U256::from(1_000_000_000_000_000_u64),
//             );
//
//         let tx_get_name = rig::utils::sign_and_encode_ethers_legacy_tx(
//             TransactionRequest::new()
//                 .to(c_addr)
//                 .gas(1 << 27)
//                 .gas_price(1000)
//                 .data(rig::utils::construct_calldata(
//                     "0x02000000",
//                     &["0x0000000000000000000000000000000000000000000000000000000000000001"],
//                 ))
//                 .nonce(0),
//             &wallet,
//         );
//
//         println!("::: Addr {:#?}", wallet.address());
//         println!("::: Byte {:0x?}", wallet.address().0);
//
//         let mut pc = rig::ProfilerConfig::new(PathBuf::from(format!(
//             "{}/os_profile_hash_keccak256.svg",
//             env!("CARGO_MANIFEST_DIR")
//         )));
//         pc.frequency_recip = 1;
//
//         let _r = chain.run_block(vec![tx_get_name], None, Some(pc));
//
//         // assert_eq!(&wallet.address().0, r.tx_results[0].as_ref().unwrap().as_returned_bytes());
//     }
// }

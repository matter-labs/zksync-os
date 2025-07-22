//!
//! This module contains bunch of standalone utility methods, useful for testing.
//!

use alloy::consensus::Transaction;
use alloy::rpc::types::TransactionRequest;
use ethers::abi::AbiEncode;
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::U256;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
pub use zksync_os_api::helpers::*;
use zksync_web3_rs::eip712::{Eip712Transaction, Eip712TransactionRequest};
use zksync_web3_rs::signers::Signer;
use zksync_web3_rs::zks_utils::EIP712_TX_TYPE;

pub use basic_system::system_implementation::flat_storage_model::{
    address_into_special_storage_key, AccountProperties, ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
};

///
/// Load wasm contract bytecode from `tests/contracts_wasm/{contract_name}`.
///
pub fn load_wasm_bytecode(contract_name: &str) -> Vec<u8> {
    let path = format!(
        "{}tests/contracts_wasm/{}/target/wasm32-unknown-unknown/release/{}.wasm",
        PathBuf::from(std::env::var("CARGO_WORKSPACE_DIR").unwrap())
            .as_os_str()
            .to_str()
            .unwrap(),
        contract_name,
        contract_name
    );
    let mut file = std::fs::File::open(path.as_str())
        .unwrap_or_else(|_| panic!("Expecting '{path}' to exist."));
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).unwrap();

    buffer
}

///
/// Load solidity contract **deployed** bytecode from `tests/instances/{project_name}` with `contract_name` name.
///
pub fn load_sol_bytecode(project_name: &str, contract_name: &str) -> Vec<u8> {
    let path = format!(
        "{}tests/contracts_sol/{}/out/{}.dep.txt",
        PathBuf::from(std::env::var("CARGO_WORKSPACE_DIR").unwrap())
            .as_os_str()
            .to_str()
            .unwrap(),
        project_name,
        contract_name,
    );

    hex::decode(
        &std::fs::read_to_string(path.as_str())
            .unwrap_or_else(|_| panic!("Expecring '{path}' to exist."))[2..],
    )
    .unwrap()
}

///
/// Creates calldata with given selector and data chunks, in fact it will just merge given hex values into byte array.
///
pub fn construct_calldata(selector: &str, data: &[&str]) -> Vec<u8> {
    let mut cd = ethers::utils::hex::decode(selector).unwrap();
    for val in data {
        let mut x = U256::from_str(val).unwrap().encode();
        cd.append(&mut x);
    }

    cd
}

#[allow(deprecated)]
pub fn encode_alloy_rpc_tx(tx: alloy::rpc::types::Transaction) -> Vec<u8> {
    use alloy::consensus::Typed2718;
    let tx_type = tx.inner.tx_type().ty();
    let from = tx.as_recovered().signer().into_array();
    let to = tx.to().map(|a| a.into_array());
    let gas_limit = tx.gas_limit() as u128;
    let (max_fee_per_gas, max_priority_fee_per_gas) = if tx_type == 2 {
        (tx.max_fee_per_gas(), tx.max_priority_fee_per_gas())
    } else {
        (tx.gas_price().unwrap(), tx.gas_price())
    };
    let nonce = tx.nonce() as u128;
    let value = tx.value().to_be_bytes();
    let data = tx.input().to_vec();
    let sig: alloy::primitives::Signature = *tx.clone().into_signed().signature();
    let mut signature = sig.as_bytes().to_vec();
    let is_eip155 = tx.inner.is_replay_protected();
    if signature[64] <= 1 {
        signature[64] += 27;
    }
    let access_list =
        tx.access_list()
            .cloned()
            .map(|access_list: alloy::rpc::types::AccessList| {
                access_list
                    .0
                    .into_iter()
                    .map(|item| {
                        let address = item.address.into_array();
                        let keys: Vec<[u8; 32]> =
                            item.storage_keys.into_iter().map(|k| k.0).collect();
                        (address, keys)
                    })
                    .collect()
            });

    #[cfg(feature = "pectra")]
    let authorization_list = tx
        .authorization_list()
        .map(|authorization_list| {
            authorization_list
                .iter()
                .map(|authorization| {
                    let auth = authorization.inner();
                    let y_parity = authorization.y_parity();
                    let r = authorization.r();
                    let s = authorization.s();
                    (
                        U256::from_big_endian(&auth.chain_id.to_be_bytes::<32>()),
                        auth.address.into_array(),
                        auth.nonce,
                        y_parity,
                        U256::from_big_endian(&r.to_be_bytes::<32>()),
                        U256::from_big_endian(&s.to_be_bytes::<32>()),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    #[cfg(not(feature = "pectra"))]
    let authorization_list = vec![];
    let reserved_dynamic =
        access_list.map(|access_list| encode_reserved_dynamic(access_list, authorization_list));

    encode_tx(
        tx_type,
        from,
        to,
        gas_limit,
        None,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        None,
        nonce,
        value,
        data,
        signature,
        None,
        reserved_dynamic,
        is_eip155,
    )
}

///
/// Sign and encode ethers legacy transaction using provided `wallet`.
///
/// It's assumed that chain id is set for wallet or tx.
///
pub fn sign_and_encode_ethers_legacy_tx(
    tx: ethers::types::TransactionRequest,
    wallet: &ethers::signers::LocalWallet,
) -> Vec<u8> {
    let tx: TypedTransaction = tx.into();
    let mut signature = wallet.sign_transaction_sync(&tx).unwrap().to_vec();
    signature[64] -= 35 + 2 * 37 - 27;
    let tx_type = 0u8;
    let from = wallet.address().0;
    let to = tx.to().map(|to| to.as_address().unwrap().0);
    let gas_limit = tx.gas().unwrap().as_u128();
    let gas_price = tx.gas_price().unwrap().as_u128();
    let nonce = tx.nonce().unwrap().as_u128();
    let mut value = [0u8; 32];
    tx.value()
        .copied()
        .unwrap_or(U256::zero())
        .to_big_endian(&mut value);
    let data = tx.data().map(|data| data.0.to_vec()).unwrap_or_default();

    encode_tx(
        tx_type, from, to, gas_limit, None, gas_price, None, None, nonce, value, data, signature,
        None, None, true,
    )
}

///
/// Sign and encode EIP-712 zkSync transaction using given wallet.
///
/// Panics if needed fields are missed or too big.
///
pub fn sign_and_encode_eip712_tx(
    tx: Eip712TransactionRequest,
    wallet: &ethers::signers::LocalWallet,
) -> Vec<u8> {
    let request = tx.clone();
    let signable_data: Eip712Transaction = request.clone().try_into().unwrap();
    // Use the correct value for gasPerPubdataByteLimit, there's a bug in the
    // zksync-web3-rs crate.
    let signable_data = signable_data.gas_per_pubdata_byte_limit(tx.custom_data.gas_per_pubdata);
    let signature: ethers::types::Signature =
        futures::executor::block_on(wallet.sign_typed_data(&signable_data))
            .expect("signing failed");

    let tx_type = EIP712_TX_TYPE;
    let from = wallet.address().0;
    let to = Some(tx.to.0);
    let gas_limit = tx.gas_limit.unwrap().as_u128();
    let gas_per_pubdata_byte_limit = Some(tx.custom_data.gas_per_pubdata.as_u128());
    let max_fee_per_gas = tx.max_fee_per_gas.unwrap().as_u128();
    let max_priority_fee_per_gas = Some(tx.max_priority_fee_per_gas.as_u128());
    let paymaster = Some(
        tx.custom_data
            .clone()
            .paymaster_params
            .map(|p| p.paymaster.0)
            .unwrap_or_default(),
    );
    let nonce = tx.nonce.as_u128();
    let mut value = [0u8; 32];
    tx.value.to_big_endian(&mut value);
    let data = tx.data.0.to_vec();
    assert!(
        tx.custom_data.factory_deps.is_empty(),
        "factory deps not supported for now"
    );
    let signature = signature.to_vec();
    let paymaster_input = Some(
        tx.custom_data
            .paymaster_params
            .map(|p| p.paymaster_input)
            .unwrap_or_default(),
    );

    encode_tx(
        tx_type,
        from,
        to,
        gas_limit,
        gas_per_pubdata_byte_limit,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        paymaster,
        nonce,
        value,
        data,
        signature,
        paymaster_input,
        None,
        true,
    )
}

///
/// Encode given request as l1 -> l2 transaction.
///
/// Panics if needed fields are unset/set incorrectly.
///
pub fn encode_l1_tx(tx: TransactionRequest) -> Vec<u8> {
    let tx_type = 255;
    let from = tx.from.unwrap().into_array();
    let to = Some(tx.to.unwrap().to().unwrap().into_array());
    let gas_limit = tx.gas.unwrap() as u128;
    let gas_per_pubdata_byte_limit = Some(0u128);
    let max_fee_per_gas = tx.max_fee_per_gas.unwrap();
    let max_priority_fee_per_gas = Some(tx.max_priority_fee_per_gas.unwrap_or_default());
    let paymaster = Some([0u8; 20]);
    let nonce = tx.nonce.unwrap() as u128;
    let value = tx.value.unwrap_or_default().to_be_bytes();
    let data = tx.input.input.unwrap_or_default().to_vec();
    let signature = vec![];
    let paymaster_input = Some(vec![]);

    encode_tx(
        tx_type,
        from,
        to,
        gas_limit,
        gas_per_pubdata_byte_limit,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        paymaster,
        nonce,
        value,
        data,
        signature,
        paymaster_input,
        None,
        true,
    )
}

#[cfg(test)]
mod tests {

    use super::U256;
    use ruint::aliases::B160;
    use zk_ee::utils::Bytes32;
    #[test]
    fn test_encode_reserved_dynamic() {
        use basic_bootloader::bootloader::transaction::reserved_dynamic_parser::ReservedDynamicParser;
        use ethers::abi::Token;
        let address0 = [0x11u8; 20];
        let address1 = [0x10u8; 20];

        let storage_keys0 = vec![[0x22u8; 32], [0x33u8; 32]];
        let storage_keys1 = vec![[0x44u8; 32], [0x55u8; 32]];

        let access_list = vec![
            (address0, storage_keys0.clone()),
            (address1, storage_keys1.clone()),
        ];

        let authorization_list = vec![
            (
                U256::from(3),
                address0,
                4,
                1,
                U256::from(42),
                U256::from(43),
            ),
            (
                U256::from(3),
                address1,
                5,
                0,
                U256::from(52),
                U256::from(53),
            ),
        ];

        let encoded_list =
            zksync_os_api::helpers::encode_reserved_dynamic(access_list, authorization_list);
        let encoded = ethers::abi::encode(&[Token::Bytes(encoded_list)]);

        // Offset is 32 to skip the initial offset for the bytes encoding
        let parser = ReservedDynamicParser::new(&encoded, 32).expect("Must create parser");
        let mut iter = parser.access_list_iter(&encoded).expect("Must create iter");
        let (address, mut keys_iter) = iter
            .next()
            .expect("Must have first")
            .expect("Must decode first");
        assert_eq!(address, B160::from_be_bytes(address0));
        let key0 = keys_iter
            .next()
            .expect("Must have key")
            .expect("Must decode key");
        assert_eq!(key0, Bytes32::from_array(storage_keys0[0]));
        let key1 = keys_iter
            .next()
            .expect("Must have key")
            .expect("Must decode key");
        assert_eq!(key1, Bytes32::from_array(storage_keys0[1]));
        assert!(keys_iter.next().is_none());

        let (address, mut keys_iter) = iter
            .next()
            .expect("Must have second")
            .expect("Must decode second");
        assert_eq!(address, B160::from_be_bytes(address1));
        let key0 = keys_iter
            .next()
            .expect("Must have key")
            .expect("Must decode key");
        assert_eq!(key0, Bytes32::from_array(storage_keys1[0]));
        let key1 = keys_iter
            .next()
            .expect("Must have key")
            .expect("Must decode key");
        assert_eq!(key1, Bytes32::from_array(storage_keys1[1]));
        assert!(keys_iter.next().is_none());

        assert!(iter.next().is_none());

        #[cfg(feature = "pectra")]
        {
            let mut iter = parser
                .authorization_list_iter(&encoded)
                .expect("Must create iter");
            let first = iter.next().expect("Must have first").expect("Must decode");
            assert_eq!(first.nonce, 4);
            assert_eq!(first.y_parity, 1);
            let second = iter.next().expect("Must have second").expect("Must decode");
            assert_eq!(second.nonce, 5);
            assert_eq!(second.y_parity, 0);
            assert!(iter.next().is_none())
        }
    }
}

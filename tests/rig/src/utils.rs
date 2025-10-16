//!
//! This module contains bunch of standalone utility methods, useful for testing.
//!

use crate::chain::BlockContext;
use crate::Chain;
use alloy::consensus::TxEip1559;
use alloy::consensus::TxEnvelope;
use alloy::primitives::Address;
use alloy::primitives::TxKind;
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use ethers::abi::AbiEncode;
use ethers::types::U256;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
pub use zksync_os_api::helpers::*;
use zksync_os_interface::traits::EncodedTx;
use zksync_os_interface::types::BlockOutput;
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
pub fn encode_alloy_rpc_tx(tx: alloy::rpc::types::Transaction) -> EncodedTx {
    let from = tx.as_recovered().signer().into_array();
    let env: TxEnvelope = tx.into();
    let bytes = encode_envelope_2718(&env);
    EncodedTx::Rlp(bytes, Address::from_slice(&from))
}

///
/// Sign and encode ethers legacy transaction using provided `wallet`.
///
/// It's assumed that chain id is set for wallet or tx.
///
pub fn sign_and_encode_ethers_legacy_tx(
    tx: ethers::types::TransactionRequest,
    wallet: &ethers::signers::LocalWallet,
) -> EncodedTx {
    use ethers::signers::Signer;
    use ethers::types::transaction::eip2718::TypedTransaction;
    let ttx: TypedTransaction = tx.into();
    let sig = wallet.sign_transaction_sync(&ttx).unwrap();
    let raw = ttx.rlp_signed(&sig);
    let from = {
        let a = wallet.address();
        Address::from_slice(a.as_bytes())
    };
    EncodedTx::Rlp(raw.to_vec(), from)
}

///
/// Sign and encode EIP-712 zkSync transaction using given wallet.
///
/// Panics if needed fields are missed or too big.
///
pub fn sign_and_encode_eip712_tx(
    tx: Eip712TransactionRequest,
    wallet: &ethers::signers::LocalWallet,
) -> EncodedTx {
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
pub fn encode_l1_tx(tx: TransactionRequest) -> EncodedTx {
    let tx_type = 0x7f;
    encode_special_tx_type(tx, tx_type)
}

///
/// Encode given request as an upgrade transaction.
///
/// Panics if needed fields are unset/set incorrectly.
///
pub fn encode_upgrade_tx(tx: TransactionRequest) -> EncodedTx {
    let tx_type = 0x7e;
    encode_special_tx_type(tx, tx_type)
}

pub fn encode_special_tx_type(tx: TransactionRequest, tx_type: u8) -> EncodedTx {
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

pub const ERC_20_BYTECODE: &str = "608060405234801561000f575f80fd5b50600436106100a7575f3560e01c806342966c681161006f57806342966c681461016557806370a082311461018157806395d89b41146101b1578063a0712d68146101cf578063a9059cbb146101eb578063dd62ed3e1461021b576100a7565b806306fdde03146100ab578063095ea7b3146100c957806318160ddd146100f957806323b872dd14610117578063313ce56714610147575b5f80fd5b6100b361024b565b6040516100c09190610985565b60405180910390f35b6100e360048036038101906100de9190610a36565b6102d7565b6040516100f09190610a8e565b60405180910390f35b6101016103c4565b60405161010e9190610ab6565b60405180910390f35b610131600480360381019061012c9190610acf565b6103c9565b60405161013e9190610a8e565b60405180910390f35b61014f61056e565b60405161015c9190610b3a565b60405180910390f35b61017f600480360381019061017a9190610b53565b610580565b005b61019b60048036038101906101969190610b7e565b610652565b6040516101a89190610ab6565b60405180910390f35b6101b9610667565b6040516101c69190610985565b60405180910390f35b6101e960048036038101906101e49190610b53565b6106f3565b005b61020560048036038101906102009190610a36565b6107c5565b6040516102129190610a8e565b60405180910390f35b61023560048036038101906102309190610ba9565b6108db565b6040516102429190610ab6565b60405180910390f35b6003805461025890610c14565b80601f016020809104026020016040519081016040528092919081815260200182805461028490610c14565b80156102cf5780601f106102a6576101008083540402835291602001916102cf565b820191905f5260205f20905b8154815290600101906020018083116102b257829003601f168201915b505050505081565b5f8160025f3373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f20819055508273ffffffffffffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffffffffff167f8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925846040516103b29190610ab6565b60405180910390a36001905092915050565b5f5481565b5f8160025f8673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f3373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8282546104519190610c71565b925050819055508160015f8673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8282546104a49190610c71565b925050819055508160015f8573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8282546104f79190610ca4565b925050819055508273ffffffffffffffffffffffffffffffffffffffff168473ffffffffffffffffffffffffffffffffffffffff167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef8460405161055b9190610ab6565b60405180910390a3600190509392505050565b60055f9054906101000a900460ff1681565b8060015f3373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8282546105cc9190610c71565b92505081905550805f808282546105e39190610c71565b925050819055505f73ffffffffffffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffffffffff167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef836040516106479190610ab6565b60405180910390a350565b6001602052805f5260405f205f915090505481565b6004805461067490610c14565b80601f01602080910402602001604051908101604052809291908181526020018280546106a090610c14565b80156106eb5780601f106106c2576101008083540402835291602001916106eb565b820191905f5260205f20905b8154815290600101906020018083116106ce57829003601f168201915b505050505081565b8060015f3373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f82825461073f9190610ca4565b92505081905550805f808282546107569190610ca4565b925050819055503373ffffffffffffffffffffffffffffffffffffffff165f73ffffffffffffffffffffffffffffffffffffffff167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef836040516107ba9190610ab6565b60405180910390a350565b5f8160015f3373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8282546108129190610c71565b925050819055508160015f8573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8282546108659190610ca4565b925050819055508273ffffffffffffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffffffffff167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef846040516108c99190610ab6565b60405180910390a36001905092915050565b6002602052815f5260405f20602052805f5260405f205f91509150505481565b5f81519050919050565b5f82825260208201905092915050565b5f5b83811015610932578082015181840152602081019050610917565b5f8484015250505050565b5f601f19601f8301169050919050565b5f610957826108fb565b6109618185610905565b9350610971818560208601610915565b61097a8161093d565b840191505092915050565b5f6020820190508181035f83015261099d818461094d565b905092915050565b5f80fd5b5f73ffffffffffffffffffffffffffffffffffffffff82169050919050565b5f6109d2826109a9565b9050919050565b6109e2816109c8565b81146109ec575f80fd5b50565b5f813590506109fd816109d9565b92915050565b5f819050919050565b610a1581610a03565b8114610a1f575f80fd5b50565b5f81359050610a3081610a0c565b92915050565b5f8060408385031215610a4c57610a4b6109a5565b5b5f610a59858286016109ef565b9250506020610a6a85828601610a22565b9150509250929050565b5f8115159050919050565b610a8881610a74565b82525050565b5f602082019050610aa15f830184610a7f565b92915050565b610ab081610a03565b82525050565b5f602082019050610ac95f830184610aa7565b92915050565b5f805f60608486031215610ae657610ae56109a5565b5b5f610af3868287016109ef565b9350506020610b04868287016109ef565b9250506040610b1586828701610a22565b9150509250925092565b5f60ff82169050919050565b610b3481610b1f565b82525050565b5f602082019050610b4d5f830184610b2b565b92915050565b5f60208284031215610b6857610b676109a5565b5b5f610b7584828501610a22565b91505092915050565b5f60208284031215610b9357610b926109a5565b5b5f610ba0848285016109ef565b91505092915050565b5f8060408385031215610bbf57610bbe6109a5565b5b5f610bcc858286016109ef565b9250506020610bdd858286016109ef565b9150509250929050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602260045260245ffd5b5f6002820490506001821680610c2b57607f821691505b602082108103610c3e57610c3d610be7565b5b50919050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b5f610c7b82610a03565b9150610c8683610a03565b9250828203905081811115610c9e57610c9d610c44565b5b92915050565b5f610cae82610a03565b9150610cb983610a03565b9250828201905080821115610cd157610cd0610c44565b5b9291505056fea2646970667358221220e7eaeda016ee21bde1fe83a42b83295125e0b6ebbba41a7b5bd87491d6bdf6ce64736f6c63430008160033";

pub const ERC_20_DEPLOYMENT_BYTECODE: &str = "60806040526040518060400160405280601381526020017f536f6c6964697479206279204578616d706c65000000000000000000000000008152506003908161004891906102f4565b506040518060400160405280600781526020017f534f4c42594558000000000000000000000000000000000000000000000000008152506004908161008d91906102f4565b50601260055f6101000a81548160ff021916908360ff1602179055503480156100b4575f80fd5b506103c3565b5f81519050919050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52604160045260245ffd5b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602260045260245ffd5b5f600282049050600182168061013557607f821691505b602082108103610148576101476100f1565b5b50919050565b5f819050815f5260205f209050919050565b5f6020601f8301049050919050565b5f82821b905092915050565b5f600883026101aa7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff8261016f565b6101b4868361016f565b95508019841693508086168417925050509392505050565b5f819050919050565b5f819050919050565b5f6101f86101f36101ee846101cc565b6101d5565b6101cc565b9050919050565b5f819050919050565b610211836101de565b61022561021d826101ff565b84845461017b565b825550505050565b5f90565b61023961022d565b610244818484610208565b505050565b5b818110156102675761025c5f82610231565b60018101905061024a565b5050565b601f8211156102ac5761027d8161014e565b61028684610160565b81016020851015610295578190505b6102a96102a185610160565b830182610249565b50505b505050565b5f82821c905092915050565b5f6102cc5f19846008026102b1565b1980831691505092915050565b5f6102e483836102bd565b9150826002028217905092915050565b6102fd826100ba565b67ffffffffffffffff811115610316576103156100c4565b5b610320825461011e565b61032b82828561026b565b5f60209050601f83116001811461035c575f841561034a578287015190505b61035485826102d9565b8655506103bb565b601f19841661036a8661014e565b5f5b828110156103915784890151825560018201915060208501945060208101905061036c565b868310156103ae57848901516103aa601f8916826102bd565b8355505b6001600288020188555050505b505050505050565b610cf3806103d05f395ff3fe608060405234801561000f575f80fd5b50600436106100a7575f3560e01c806342966c681161006f57806342966c681461016557806370a082311461018157806395d89b41146101b1578063a0712d68146101cf578063a9059cbb146101eb578063dd62ed3e1461021b576100a7565b806306fdde03146100ab578063095ea7b3146100c957806318160ddd146100f957806323b872dd14610117578063313ce56714610147575b5f80fd5b6100b361024b565b6040516100c0919061096b565b60405180910390f35b6100e360048036038101906100de9190610a1c565b6102d7565b6040516100f09190610a74565b60405180910390f35b6101016103c4565b60405161010e9190610a9c565b60405180910390f35b610131600480360381019061012c9190610ab5565b6103c9565b60405161013e9190610a74565b60405180910390f35b61014f61056e565b60405161015c9190610b20565b60405180910390f35b61017f600480360381019061017a9190610b39565b610580565b005b61019b60048036038101906101969190610b64565b610652565b6040516101a89190610a9c565b60405180910390f35b6101b9610667565b6040516101c6919061096b565b60405180910390f35b6101e960048036038101906101e49190610b39565b6106f3565b005b61020560048036038101906102009190610a1c565b6107c5565b6040516102129190610a74565b60405180910390f35b61023560048036038101906102309190610b8f565b6108db565b6040516102429190610a9c565b60405180910390f35b6003805461025890610bfa565b80601f016020809104026020016040519081016040528092919081815260200182805461028490610bfa565b80156102cf5780601f106102a6576101008083540402835291602001916102cf565b820191905f5260205f20905b8154815290600101906020018083116102b257829003601f168201915b505050505081565b5f8160025f3373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f20819055508273ffffffffffffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffffffffff167f8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925846040516103b29190610a9c565b60405180910390a36001905092915050565b5f5481565b5f8160025f8673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f3373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8282546104519190610c57565b925050819055508160015f8673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8282546104a49190610c57565b925050819055508160015f8573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8282546104f79190610c8a565b925050819055508273ffffffffffffffffffffffffffffffffffffffff168473ffffffffffffffffffffffffffffffffffffffff167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef8460405161055b9190610a9c565b60405180910390a3600190509392505050565b60055f9054906101000a900460ff1681565b8060015f3373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8282546105cc9190610c57565b92505081905550805f808282546105e39190610c57565b925050819055505f73ffffffffffffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffffffffff167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef836040516106479190610a9c565b60405180910390a350565b6001602052805f5260405f205f915090505481565b6004805461067490610bfa565b80601f01602080910402602001604051908101604052809291908181526020018280546106a090610bfa565b80156106eb5780601f106106c2576101008083540402835291602001916106eb565b820191905f5260205f20905b8154815290600101906020018083116106ce57829003601f168201915b505050505081565b8060015f3373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f82825461073f9190610c8a565b92505081905550805f808282546107569190610c8a565b925050819055503373ffffffffffffffffffffffffffffffffffffffff165f73ffffffffffffffffffffffffffffffffffffffff167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef836040516107ba9190610a9c565b60405180910390a350565b5f8160015f3373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8282546108129190610c57565b925050819055508160015f8573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020015f205f8282546108659190610c8a565b925050819055508273ffffffffffffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffffffffff167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef846040516108c99190610a9c565b60405180910390a36001905092915050565b6002602052815f5260405f20602052805f5260405f205f91509150505481565b5f81519050919050565b5f82825260208201905092915050565b8281835e5f83830152505050565b5f601f19601f8301169050919050565b5f61093d826108fb565b6109478185610905565b9350610957818560208601610915565b61096081610923565b840191505092915050565b5f6020820190508181035f8301526109838184610933565b905092915050565b5f80fd5b5f73ffffffffffffffffffffffffffffffffffffffff82169050919050565b5f6109b88261098f565b9050919050565b6109c8816109ae565b81146109d2575f80fd5b50565b5f813590506109e3816109bf565b92915050565b5f819050919050565b6109fb816109e9565b8114610a05575f80fd5b50565b5f81359050610a16816109f2565b92915050565b5f8060408385031215610a3257610a3161098b565b5b5f610a3f858286016109d5565b9250506020610a5085828601610a08565b9150509250929050565b5f8115159050919050565b610a6e81610a5a565b82525050565b5f602082019050610a875f830184610a65565b92915050565b610a96816109e9565b82525050565b5f602082019050610aaf5f830184610a8d565b92915050565b5f805f60608486031215610acc57610acb61098b565b5b5f610ad9868287016109d5565b9350506020610aea868287016109d5565b9250506040610afb86828701610a08565b9150509250925092565b5f60ff82169050919050565b610b1a81610b05565b82525050565b5f602082019050610b335f830184610b11565b92915050565b5f60208284031215610b4e57610b4d61098b565b5b5f610b5b84828501610a08565b91505092915050565b5f60208284031215610b7957610b7861098b565b5b5f610b86848285016109d5565b91505092915050565b5f8060408385031215610ba557610ba461098b565b5b5f610bb2858286016109d5565b9250506020610bc3858286016109d5565b9150509250929050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52602260045260245ffd5b5f6002820490506001821680610c1157607f821691505b602082108103610c2457610c23610bcd565b5b50919050565b7f4e487b71000000000000000000000000000000000000000000000000000000005f52601160045260245ffd5b5f610c61826109e9565b9150610c6c836109e9565b9250828203905081811115610c8457610c83610c2a565b5b92915050565b5f610c94826109e9565b9150610c9f836109e9565b9250828201905080821115610cb757610cb6610c2a565b5b9291505056fea26469706673582212204d7564c0b3573c75568bc54dffc602c3bf6db07b9815fa5f2fa92d7ad7d2a7a764736f6c63430008190033";
pub const ERC_20_MINT_CALLDATA: &str =
    "a0712d6800000000000000000000000000000000000000000000000000000000000f4240";
pub const ERC_20_TRANSFER_CALLDATA: &str = "a9059cbb000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000003e8";

pub fn compute_erc20_balance_slot(address: alloy::primitives::Address) -> ruint::aliases::U256 {
    let mut buf = [0u8; 64];
    address
        .0
        .iter()
        .rev()
        .enumerate()
        .for_each(|(i, b)| buf[31 - i] = *b);
    buf[63] = 1u8;
    let hash = alloy::primitives::keccak256(buf);
    ruint::aliases::U256::from_be_bytes(hash.0)
}

pub fn run_block_of_erc20<const RANDOMIZED: bool>(
    chain: &mut Chain<RANDOMIZED>,
    n: usize,
    block_context: Option<BlockContext>,
) -> BlockOutput {
    let wallets: Vec<_> = (1..=n).map(|_| PrivateKeySigner::random()).collect();
    let dsts: Vec<_> = (1..=n)
        .map(|i| {
            let hex = format!("{i:04x}");
            let repeated = hex.repeat(40 / hex.len());
            let array: [u8; 20] = hex::decode(repeated).unwrap().try_into().unwrap();
            alloy::primitives::Address::from(array)
        })
        .collect();

    // If base fee is zero, we can avoid paying priority fee.
    let max_priority_fee_per_gas = if block_context
        .as_ref()
        .is_some_and(|bc| bc.eip1559_basefee.is_zero())
    {
        0
    } else {
        1000
    };

    let transactions: Vec<_> = wallets
        .iter()
        .zip(dsts.clone())
        .map(|(wallet, to)| {
            let transfer_tx = TxEip1559 {
                chain_id: 37u64,
                nonce: 0,
                max_fee_per_gas: 1000,
                max_priority_fee_per_gas,
                gas_limit: 60_000,
                to: TxKind::Call(to),
                value: Default::default(),
                access_list: Default::default(),
                input: hex::decode(ERC_20_TRANSFER_CALLDATA).unwrap().into(),
            };
            sign_and_encode_alloy_tx(transfer_tx, wallet)
        })
        .collect();

    let bytecode = hex::decode(ERC_20_BYTECODE).unwrap();

    dsts.iter().for_each(|to| {
        chain.set_evm_bytecode(
            ruint::aliases::B160::from_be_bytes(to.into_array()),
            &bytecode,
        );
    });

    wallets.iter().zip(dsts.clone()).for_each(|(wallet, to)| {
        chain.set_balance(
            ruint::aliases::B160::from_be_bytes(wallet.address().0 .0),
            ruint::aliases::U256::from(1_000_000_000_000_000_u64),
        );
        let key = compute_erc20_balance_slot(wallet.address());
        let value =
            ruint::aliases::B256::from(ruint::aliases::U256::from(1_000_000_000_000_000_u64));
        chain.set_storage_slot(ruint::aliases::B160::from_be_bytes(to.0 .0), key, value)
    });

    let output = chain.run_block(transactions, block_context, None);
    assert!(output.tx_results.iter().cloned().enumerate().all(|(i, r)| {
        let success = r.clone().is_ok_and(|o| o.is_success());
        if !success {
            println!("Transaction {i} failed with: {r:?}")
        }
        success
    }));
    output
}

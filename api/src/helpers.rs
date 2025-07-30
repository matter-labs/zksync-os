use alloy::consensus::SignableTransaction;
use alloy::consensus::TypedTransaction;
use alloy::network::TxSignerSync;
#[allow(deprecated)]
use alloy::primitives::Signature;
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use basic_system::system_implementation::flat_storage_model::bytecode_padding_len;
use basic_system::system_implementation::flat_storage_model::AccountProperties;
use ethers::abi::{Token, Uint};
use ethers::types::U256;
use forward_system::run::PreimageSource;
use std::alloc::Global;
use std::ops::Add;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::EIP7702_DELEGATION_MARKER;
use zk_ee::utils::Bytes32;

// Getters

/// Retrieves balance from an account.
pub fn get_balance(account: &AccountProperties) -> ruint::aliases::U256 {
    account.balance
}

/// Retrieves nonce from an account.
pub fn get_nonce(account: &AccountProperties) -> u64 {
    account.nonce
}

/// Get unpadded code from full bytecode with artifacts.
pub fn get_unpadded_code<'a>(full_bytecode: &'a [u8], account: &AccountProperties) -> &'a [u8] {
    &full_bytecode[0..account.unpadded_code_len as usize]
}

/// Retrieves code for an account.
/// This function returns unpadded code, without artifacts.
pub fn get_code<P: PreimageSource>(
    preimage_source: &mut P,
    account: &AccountProperties,
) -> Vec<u8> {
    match preimage_source.get_preimage(account.bytecode_hash) {
        None => vec![],
        Some(full_bytecode) => get_unpadded_code(&full_bytecode, account).to_vec(),
    }
}

/// Sets the balance for an account.
pub fn set_properties_balance(account: &mut AccountProperties, balance: ruint::aliases::U256) {
    account.balance = balance
}

/// Sets the nonce for an account.
pub fn set_properties_nonce(account: &mut AccountProperties, nonce: u64) {
    account.nonce = nonce
}

/// Sets a given [evm_code] for an [account].
/// Computes artifacts for [evm_code] and returns the extended
/// bytecode (code + artifacts).
pub fn set_properties_code(account: &mut AccountProperties, evm_code: &[u8]) -> Vec<u8> {
    use crypto::blake2s::Blake2s256;
    use crypto::sha3::Keccak256;
    use crypto::MiniDigest;

    let is_delegation = evm_code.len() >= 3 && evm_code[0..3] == EIP7702_DELEGATION_MARKER;

    let unpadded_code_len = evm_code.len();

    let observable_bytecode_hash = Bytes32::from_array(Keccak256::digest(evm_code));

    let (bytecode_hash, artifacts_len, full_bytecode) = if is_delegation {
        let artifacts_len = 0;
        let padding_len = bytecode_padding_len(unpadded_code_len);
        let full_len = unpadded_code_len + padding_len + artifacts_len;
        let mut padded_bytecode: Vec<u8> = vec![0u8; full_len];
        padded_bytecode[..unpadded_code_len].copy_from_slice(evm_code);
        let bytecode_hash = Bytes32::from_array(Blake2s256::digest(&padded_bytecode));

        account.versioning_data.set_as_delegated();

        (bytecode_hash, artifacts_len, padded_bytecode)
    } else {
        let artifacts =
            evm_interpreter::BytecodePreprocessingData::create_artifacts_inner(Global, evm_code);
        let artifacts = artifacts.as_slice();
        let artifacts_len = artifacts.len();
        let padding_len = bytecode_padding_len(unpadded_code_len);
        let full_len = unpadded_code_len + padding_len + artifacts_len;
        let mut bytecode_and_artifacts: Vec<u8> = vec![0u8; full_len];
        bytecode_and_artifacts[..unpadded_code_len].copy_from_slice(evm_code);
        let bitmap_offset = unpadded_code_len + padding_len;
        bytecode_and_artifacts[bitmap_offset..].copy_from_slice(artifacts);

        let bytecode_hash = Bytes32::from_array(Blake2s256::digest(&bytecode_and_artifacts));

        account
            .versioning_data
            .set_code_version(evm_interpreter::ARTIFACTS_CACHING_CODE_VERSION_BYTE);
        account.versioning_data.set_as_deployed();

        (bytecode_hash, artifacts_len, bytecode_and_artifacts)
    };

    account.observable_bytecode_hash = observable_bytecode_hash;
    account.bytecode_hash = bytecode_hash;
    account
        .versioning_data
        .set_ee_version(ExecutionEnvironmentType::EVM as u8);
    account.unpadded_code_len = unpadded_code_len as u32;
    account.artifacts_len = artifacts_len as u32;
    account.observable_bytecode_len = unpadded_code_len as u32;
    full_bytecode
}

pub fn encode_reserved_dynamic(
    access_list: Vec<([u8; 20], Vec<[u8; 32]>)>,
    authorization_list: Vec<(U256, [u8; 20], u64, u8, U256, U256)>,
) -> Vec<u8> {
    let access_list: Vec<Token> = access_list
        .into_iter()
        .map(|(addr, keys)| {
            let address_token = Token::Address(addr.into());
            let keys_token = Token::Array(
                keys.into_iter()
                    .map(|k| Token::FixedBytes(k.to_vec()))
                    .collect(),
            );
            Token::Tuple(vec![address_token, keys_token])
        })
        .collect();

    let authorization_list: Vec<Token> = authorization_list
        .into_iter()
        .map(|(chain_id, address, nonce, y_parity, r, s)| {
            let chain_id = Token::Uint(chain_id);
            let address = Token::Address(address.into());
            let nonce = Token::Uint(U256::from(nonce));
            let y_parity = Token::Uint(U256::from(y_parity));
            let r = Token::Uint(r);
            let s = Token::Uint(s);
            Token::Tuple(vec![chain_id, address, nonce, y_parity, r, s])
        })
        .collect();

    // 2-element list to be able to extend reserved_dynamic
    let outer = Token::Array(vec![
        Token::Array(access_list),
        Token::Array(authorization_list),
    ]);
    ethers::abi::encode(&[outer])
}

///
/// Internal tx encoding method.
///
#[allow(clippy::too_many_arguments)]
pub fn encode_tx(
    tx_type: u8,
    from: [u8; 20],
    to: Option<[u8; 20]>,
    gas_limit: u128,
    gas_per_pubdata_byte_limit: Option<u128>,
    max_fee_per_gas: u128,
    max_priority_fee_per_gas: Option<u128>,
    paymaster: Option<[u8; 20]>,
    nonce: u128,
    value: [u8; 32],
    data: Vec<u8>,
    signature: Vec<u8>,
    paymaster_input: Option<Vec<u8>>,
    reserved_dynamic: Option<Vec<u8>>,
    is_eip155: bool,
) -> Vec<u8> {
    fn address_to_uint(address: &[u8; 20]) -> Uint {
        let mut padded = [0u8; 32];
        padded[12..].copy_from_slice(address.as_slice());
        Uint::from(padded)
    }

    ethers::abi::encode(&[
        Token::Uint(tx_type.into()),
        Token::Uint(address_to_uint(&from)),
        Token::Uint(address_to_uint(&to.unwrap_or_default())),
        Token::Uint(gas_limit.into()),
        Token::Uint(gas_per_pubdata_byte_limit.unwrap_or_default().into()),
        Token::Uint(max_fee_per_gas.into()),
        Token::Uint(max_priority_fee_per_gas.unwrap_or(max_fee_per_gas).into()),
        Token::Uint(address_to_uint(&paymaster.unwrap_or_default())),
        Token::Uint(U256::from(nonce)),
        Token::Uint(U256::from(value)),
        Token::FixedArray(vec![
            Token::Uint(if tx_type == 0 {
                if is_eip155 {
                    U256::one()
                } else {
                    U256::zero()
                }
            } else if tx_type == 255 {
                U256::from(value).add(gas_limit * max_fee_per_gas)
            } else {
                U256::zero()
            }),
            Token::Uint(if to.is_none() {
                U256::one()
            } else {
                U256::zero()
            }),
            Token::Uint(U256::zero()),
            Token::Uint(U256::zero()),
        ]),
        Token::Bytes(data),
        Token::Bytes(signature),
        // factory deps not supported for now
        Token::Array(vec![]),
        Token::Bytes(paymaster_input.unwrap_or_default()),
        Token::Bytes(reserved_dynamic.unwrap_or_default()),
    ])
    .to_vec()
}

///
/// Sign and encode alloy transaction using provided `wallet`.
///
#[allow(deprecated)]
pub fn sign_and_encode_alloy_tx(
    mut tx: impl SignableTransaction<Signature>,
    wallet: &PrivateKeySigner,
) -> Vec<u8> {
    let mut signature = wallet
        .sign_transaction_sync(&mut tx)
        .unwrap()
        .as_bytes()
        .to_vec();

    // seems that it's a case for the legacy txs
    if signature[64] <= 1 {
        signature[64] += 27;
    }
    let tx_type = tx.ty();
    let from = wallet.address().into_array();
    let to = tx.to().map(|to| to.into_array());
    let gas_limit = tx.gas_limit() as u128;
    let max_fee_per_gas = tx.max_fee_per_gas();
    let max_priority_fee_per_gas = tx.max_priority_fee_per_gas();
    let nonce = tx.nonce() as u128;
    let value = tx.value().to_be_bytes();
    let data = tx.input().to_vec();

    let access_list = tx
        .access_list()
        .map(|access_list: &alloy::rpc::types::AccessList| {
            access_list
                .clone()
                .0
                .into_iter()
                .map(|item| {
                    let address = item.address.into_array();
                    let keys: Vec<[u8; 32]> = item.storage_keys.into_iter().map(|k| k.0).collect();
                    (address, keys)
                })
                .collect()
        });

    let authorization_list = tx.authorization_list().map(|authorization_list| {
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
    });
    let reserved_dynamic = access_list.map(|access_list| {
        encode_reserved_dynamic(access_list, authorization_list.unwrap_or_default())
    });

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
        true,
    )
}

///
/// Sign and encode alloy transaction request using provided `wallet`.
///
pub fn sign_and_encode_transaction_request(
    req: TransactionRequest,
    wallet: &PrivateKeySigner,
) -> Vec<u8> {
    let typed_tx = req.build_typed_tx().expect("Failed to build typed tx");
    match typed_tx {
        TypedTransaction::Legacy(tx) => sign_and_encode_alloy_tx(tx, wallet),
        TypedTransaction::Eip1559(tx) => sign_and_encode_alloy_tx(tx, wallet),
        TypedTransaction::Eip7702(tx) => sign_and_encode_alloy_tx(tx, wallet),
        TypedTransaction::Eip2930(tx) => sign_and_encode_alloy_tx(tx, wallet),
        TypedTransaction::Eip4844(_) => panic!("Unsupported tx type"),
    }
}

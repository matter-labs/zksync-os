use alloy::consensus::{EthereumTxEnvelope, SignableTransaction};
use alloy::consensus::{Signed, TxEnvelope, TypedTransaction};
use alloy::dyn_abi::DynSolValue;
use alloy::network::TxSignerSync;
use alloy::primitives::Signature;
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use basic_system::system_implementation::flat_storage_model::bytecode_padding_len;
use basic_system::system_implementation::flat_storage_model::AccountProperties;
use forward_system::run::PreimageSource;
use ruint::aliases::U256;
use std::alloc::Global;
use std::ops::Add;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::EIP7702_DELEGATION_MARKER;
use zk_ee::utils::Bytes32;
use zksync_os_interface::traits::EncodedTx;

// Getters

/// Retrieves balance from an account.
pub fn get_balance(account: &AccountProperties) -> U256 {
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
pub fn set_properties_balance(account: &mut AccountProperties, balance: U256) {
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
) -> EncodedTx {
    fn address_to_value(address: &[u8; 20]) -> DynSolValue {
        let mut padded = [0u8; 32];
        padded[12..].copy_from_slice(address.as_slice());
        U256::from_be_bytes(padded).into()
    }

    let bytes = DynSolValue::Tuple(vec![
        U256::from(tx_type).into(),
        address_to_value(&from),
        address_to_value(&to.unwrap_or_default()),
        U256::from(gas_limit).into(),
        gas_per_pubdata_byte_limit.unwrap_or_default().into(),
        max_fee_per_gas.into(),
        max_priority_fee_per_gas.unwrap_or(max_fee_per_gas).into(),
        address_to_value(&paymaster.unwrap_or_default()),
        U256::from(nonce).into(),
        U256::from_be_bytes(value).into(),
        DynSolValue::FixedArray(vec![
            (if tx_type == 0 {
                if is_eip155 {
                    U256::ONE
                } else {
                    U256::ZERO
                }
            } else if tx_type == 0x7f {
                U256::from_be_bytes(value).add(U256::from(gas_limit * max_fee_per_gas))
            } else {
                U256::ZERO
            })
            .into(),
            (if to.is_none() { U256::ONE } else { U256::ZERO }).into(),
            U256::ZERO.into(),
            U256::ZERO.into(),
        ]),
        DynSolValue::Bytes(data),
        DynSolValue::Bytes(signature),
        // factory deps not supported for now
        DynSolValue::Array(vec![]),
        DynSolValue::Bytes(paymaster_input.unwrap_or_default()),
        DynSolValue::Bytes(reserved_dynamic.unwrap_or_default()),
    ])
    .abi_encode_params();
    EncodedTx::Abi(bytes)
}

///
/// Sign and encode alloy transaction using provided `wallet`.
///
pub fn sign_and_encode_alloy_tx<T>(mut tx: T, wallet: &PrivateKeySigner) -> EncodedTx
where
    T: SignableTransaction<Signature>,
    Signed<T>: Into<TxEnvelope>,
{
    let sig: Signature = wallet
        .sign_transaction_sync(&mut tx)
        .expect("transaction signing failed");
    let signed: Signed<T> = tx.into_signed(sig);
    let env: TxEnvelope = signed.into();
    let bytes = encode_envelope_2718(&env);
    EncodedTx::Rlp(bytes, wallet.address())
}

pub fn encode_envelope_2718(env: &TxEnvelope) -> Vec<u8> {
    let mut out = Vec::new();
    match env {
        EthereumTxEnvelope::Legacy(signed) => {
            signed.rlp_encode(&mut out);
        }
        EthereumTxEnvelope::Eip2930(signed) => {
            out.push(0x01);
            signed.rlp_encode(&mut out);
        }
        EthereumTxEnvelope::Eip1559(signed) => {
            out.push(0x02);
            signed.rlp_encode(&mut out);
        }
        EthereumTxEnvelope::Eip7702(signed) => {
            out.push(0x04);
            signed.rlp_encode(&mut out);
        }
        _ => unimplemented!(),
    }
    out
}

///
/// Sign and encode alloy transaction request using provided `wallet`.
///
pub fn sign_and_encode_transaction_request(
    req: TransactionRequest,
    wallet: &PrivateKeySigner,
) -> EncodedTx {
    let typed_tx = req.build_typed_tx().expect("Failed to build typed tx");
    match typed_tx {
        TypedTransaction::Legacy(tx) => sign_and_encode_alloy_tx(tx, wallet),
        TypedTransaction::Eip1559(tx) => sign_and_encode_alloy_tx(tx, wallet),
        TypedTransaction::Eip7702(tx) => sign_and_encode_alloy_tx(tx, wallet),
        TypedTransaction::Eip2930(tx) => sign_and_encode_alloy_tx(tx, wallet),
        TypedTransaction::Eip4844(_) => panic!("Unsupported tx type"),
    }
}

use basic_system::system_implementation::flat_storage_model::bytecode_padding_len;
use basic_system::system_implementation::flat_storage_model::AccountProperties;
use forward_system::run::PreimageSource;
use ruint::aliases::U256;
use std::alloc::Global;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::utils::Bytes32;

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

    let unpadded_code_len = evm_code.len();
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

    let observable_bytecode_hash = Bytes32::from_array(Keccak256::digest(evm_code));
    let bytecode_hash = Bytes32::from_array(Blake2s256::digest(&bytecode_and_artifacts));

    account.observable_bytecode_hash = observable_bytecode_hash;
    account.bytecode_hash = bytecode_hash;
    account.versioning_data.set_as_deployed();
    account
        .versioning_data
        .set_ee_version(ExecutionEnvironmentType::EVM as u8);
    account
        .versioning_data
        .set_code_version(evm_interpreter::ARTIFACTS_CACHING_CODE_VERSION_BYTE);
    account.unpadded_code_len = unpadded_code_len as u32;
    account.artifacts_len = artifacts_len as u32;
    account.observable_bytecode_len = unpadded_code_len as u32;

    bytecode_and_artifacts
}

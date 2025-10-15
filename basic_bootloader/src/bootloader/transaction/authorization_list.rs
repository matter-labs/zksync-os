//!
//! Parser and logic for authorization lists.
//!

use crate::bootloader::errors::InvalidTransaction;
use crate::bootloader::BootloaderSubsystemError;
use core::fmt::Write;
use crypto::MiniDigest;
use ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::ArrayBuilder;
use zk_ee::system::errors::interface::InterfaceError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::IOSubsystem;
use zk_ee::system::NonceError;
use zk_ee::system::{AccountDataRequest, EthereumLikeTypes, IOSubsystemExt, Resources, System};
use zk_ee::{internal_error, wrap_error};

use super::rlp_encoded::AuthorizationList;
use super::TxError;

pub fn parse_authorization_list_and_apply_delegations<S: EthereumLikeTypes>(
    system: &mut System<S>,
    resources: &mut S::Resources,
    auth_list: AuthorizationList<'_>,
) -> Result<(), TxError>
where
    S::IO: IOSubsystemExt,
{
    use crate::bootloader::transaction::rlp_encoded::AuthorizationEntry;
    let mut hasher = crypto::sha3::Keccak256::new();

    for entry in auth_list.iter() {
        let AuthorizationEntry {
            chain_id,
            address,
            nonce,
            y_parity,
            r,
            s,
        } = entry;
        let success = validate_and_apply_delegation(
            system,
            resources,
            &chain_id,
            nonce,
            address,
            (y_parity, r, s),
            &mut hasher,
        )?;
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Delegation success: {success}\n"));

        if !success {}
    }
    Ok(())
}

// Magic byte from EIP-7702
const EIP7702_MAGIC: u8 = 0x05;

/// Validate and apply an authorization list item, following EIP-7702:
/// 1. Verify the chain ID is 0 or the ID of the current chain.
/// 2. Verify the nonce is less than 2**64 - 1.
/// 3. Let authority = ecrecover(msg, y_parity, r, s).
///    Where msg = keccak(EIP7702_MAGIC || rlp([chain_id, address, nonce])).
///    Verify s is less than or equal to secp256k1n/2.
/// 4. Warm up authority
/// 5. Verify the authority is not a contract.
/// 6. Verify the nonce of authority is equal to nonce.
/// 7. Add refund if authority isn't empty.
/// 8. Set the code of authority to be 0xef0100 || address.
///    If address is 0x0, clear the account’s code
///    (and deployment status) instead.
/// 9. Increase the nonce of authority by one.
///
/// Note that if any of these checks fail, the function returns
/// false.
#[inline]
fn validate_and_apply_delegation<S: EthereumLikeTypes>(
    system: &mut System<S>,
    resources: &mut S::Resources,
    auth_chain_id: &U256,
    auth_nonce: u64,
    delegation_address: &[u8; 20],
    auth_sig_data: (u8, &[u8], &[u8]),
    hasher: &mut crypto::sha3::Keccak256,
) -> Result<bool, TxError>
where
    S::IO: IOSubsystemExt,
{
    let chain_id = system.get_chain_id();
    // 1. Check chain id
    if !auth_chain_id.is_zero() && auth_chain_id != &U256::from(chain_id) {
        return Ok(false);
    }
    // 2. Check for nonce overflow
    if auth_nonce == u64::MAX {
        return Ok(false);
    }
    // 3. Signature
    // EIP-2 check
    let (_, _, auth_s) = auth_sig_data;
    let s = U256::try_from_be_slice(auth_s)
        .ok_or::<TxError>(InvalidTransaction::InvalidStructure.into())?;
    if s > crypto::secp256k1::SECP256K1N_HALF_U256 {
        return Ok(false);
    }
    let msg = resources.with_infinite_ergs(|inf_ergs| {
        compute_auth_message_signed_hash::<S>(
            inf_ergs,
            auth_chain_id,
            auth_nonce,
            delegation_address,
            hasher,
        )
    })?;
    let Some(authority) = resources
        .with_infinite_ergs(|inf_ergs| recover_authority(system, inf_ergs, auth_sig_data, &msg))?
    else {
        return Ok(false);
    };

    // 4. Read authority account
    // Gas already charged in intrinsic
    let account_properties = resources.with_infinite_ergs(|inf_ergs| {
        system.io.read_account_properties(
            ExecutionEnvironmentType::NoEE,
            inf_ergs,
            &authority,
            AccountDataRequest::empty()
                .with_nonce()
                .with_nominal_token_balance()
                .with_is_delegated()
                .with_artifacts_len()
                .with_unpadded_code_len(),
        )
    })?;
    // 5. Check authority is not a contract
    if account_properties.is_contract() {
        return Ok(false);
    }
    // 6. Check nonce
    if account_properties.nonce.0 != auth_nonce {
        return Ok(false);
    }
    // 7. Add refund if authority is not empty.
    #[cfg(feature = "evm_refunds")]
    {
        let is_empty = account_properties.nonce.0 == 0
            && account_properties.unpadded_code_len.0 == 0
            && account_properties.nominal_token_balance.0.is_zero();
        if !is_empty {
            system.io.add_evm_refund(
                (evm_interpreter::gas_constants::NEWACCOUNT
                    - evm_interpreter::gas_constants::PER_AUTH_BASE_COST) as u32,
            )?
        }
    }
    let delegation_address = B160::from_be_bytes(*delegation_address);
    let _ = system.get_logger().write_fmt(format_args!(
        "Will delegate address 0x{:040x} -> 0x{:040x}\n",
        authority.as_uint(),
        delegation_address.as_uint()
    ));

    // 8. Set code for authority, system function
    //    will handle the two cases (unsetting).
    resources.with_infinite_ergs(|inf_ergs| {
        system
            .io
            .set_delegation(inf_ergs, &authority, &delegation_address)
    })?;
    // 9.Bump nonce
    resources
        .with_infinite_ergs(|inf_ergs| {
            system
                .io
                .increment_nonce(ExecutionEnvironmentType::NoEE, inf_ergs, &authority, 1)
        })
        .map_err(|e| -> BootloaderSubsystemError {
            match e {
                SubsystemError::LeafUsage(InterfaceError(NonceError::NonceOverflow, _)) => {
                    internal_error!("Cannot overflow, already checked").into()
                }
                _ => wrap_error!(e),
            }
        })?;
    Ok(true)
}

fn compute_auth_message_signed_hash<S: EthereumLikeTypes>(
    resources: &mut S::Resources,
    auth_chain_id: &U256,
    auth_nonce: u64,
    delegation_address: &[u8; 20],
    hasher: &mut crypto::sha3::Keccak256,
) -> Result<[u8; 32], TxError> {
    use crate::bootloader::rlp;

    let list_payload_len = rlp::estimate_number_encoding_len(&auth_chain_id.to_be_bytes::<32>())
        + rlp::ADDRESS_ENCODING_LEN
        + rlp::estimate_number_encoding_len(&auth_nonce.to_be_bytes());
    let total_list_len = rlp::estimate_length_encoding_len(list_payload_len) + list_payload_len;
    let encoding_len = 1 + total_list_len;
    crate::bootloader::transaction::charge_keccak(encoding_len, resources)?;
    hasher.update([EIP7702_MAGIC]);
    rlp::apply_list_length_encoding_to_hash(list_payload_len, hasher);
    rlp::apply_number_encoding_to_hash(&auth_chain_id.to_be_bytes::<32>(), hasher);
    rlp::apply_bytes_encoding_to_hash(delegation_address, hasher);
    rlp::apply_number_encoding_to_hash(&auth_nonce.to_be_bytes(), hasher);

    Ok(hasher.finalize_reset())
}

fn recover_authority<S: EthereumLikeTypes>(
    system: &mut System<S>,
    resources: &mut S::Resources,
    auth_sig_data: (u8, &[u8], &[u8]),
    msg: &[u8; 32],
) -> Result<Option<B160>, TxError> {
    use zk_ee::system::SystemFunctions;
    let mut ecrecover_input = [0u8; 128];
    let (parity, r, s) = auth_sig_data;
    ecrecover_input[0..32].copy_from_slice(msg);
    ecrecover_input[63] = if parity <= 1 { parity + 27 } else { parity };
    ecrecover_input[64..96][(32 - r.len())..].copy_from_slice(r);
    ecrecover_input[96..128][(32 - s.len())..].copy_from_slice(s);
    let mut ecrecover_output = ArrayBuilder::default();
    // Recover is counted in intrinsic gas
    resources
        .with_infinite_ergs(|inf_ergs| {
            S::SystemFunctions::secp256k1_ec_recover(
                ecrecover_input.as_slice(),
                &mut ecrecover_output,
                inf_ergs,
                system.get_allocator(),
            )
        })
        .map_err(SystemError::from)?;
    if ecrecover_output.is_empty() {
        Ok(None)
    } else {
        Ok(Some(
            B160::try_from_be_slice(&ecrecover_output.build()[12..])
                .ok_or(internal_error!("Invalid ecrecover return value"))?,
        ))
    }
}

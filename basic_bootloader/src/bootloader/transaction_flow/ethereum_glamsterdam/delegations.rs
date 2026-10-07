//! The EIP-7702 delegations of a Glamsterdam transaction (`set_delegation` of the
//! specification): applied after the fee is charged, with their state-dependent costs on
//! top of the intrinsic `EXECUTION_PER_AUTH_BASE_COST`: the creation of an authority that
//! does not exist (state gas), the first write of an authority the transaction does not
//! pay for elsewhere (`ACCOUNT_WRITE`), and the delegation indicator of an authority that
//! had none (state gas, never refilled).

use super::{is_alive, out_of_gas, PreparationError};
use crate::bootloader::supported_ees::EthereumLikeSystem;
use crate::bootloader::transaction::authorization_list::{
    compute_auth_message_signed_hash, recover_authority,
};
use crate::bootloader::transaction::rlp_encoded::{AuthorizationEntry, AuthorizationList};
use crate::bootloader::TxError;
use alloc::vec::Vec;
use core::fmt::Write;
use crypto::MiniDigest;
use evm_interpreter::v3::gas::{ACCOUNT_WRITE, AUTH_BASE, NEW_ACCOUNT};
use ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::interface::InterfaceError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::{
    AccountDataRequest, IOSubsystem, IOSubsystemExt, NonceError, Resources, StateGasResources,
    System,
};
use zk_ee::system_log;
use zk_ee::{internal_error, wrap_error};

/// Applies the valid authorizations in order. `value_recipient` is the recipient of a
/// value transfer, whose write the transaction pays in `TX_VALUE_COST`; the sender's is
/// in `TX_BASE`.
pub(super) fn apply_delegations<S: EthereumLikeSystem>(
    system: &mut System<S>,
    resources: &mut S::Resources,
    auth_list: AuthorizationList<'_>,
    sender: &B160,
    value_recipient: Option<B160>,
) -> Result<(), PreparationError>
where
    S::IO: IOSubsystemExt,
    S::Resources: StateGasResources,
{
    let allocator = system.get_allocator();
    let mut hasher = crypto::sha3::Keccak256::new();
    // every list holds at most one entry per authorization (the proving-target allocator
    // cannot grow a buffer)
    let capacity = auth_list.iter().count() + 2;
    // the authorities whose write the transaction paid already
    let mut paid_writes: Vec<B160, S::Allocator> =
        Vec::with_capacity_in(capacity, allocator.clone());
    paid_writes.push(*sender);
    if let Some(recipient) = value_recipient {
        paid_writes.push(recipient);
    }
    // the authorities seen, with whether they were delegated before the transaction
    let mut seen: Vec<(B160, bool), S::Allocator> =
        Vec::with_capacity_in(capacity, allocator.clone());
    // the authorities a delegation was set for in this transaction
    let mut delegation_set_for: Vec<B160, S::Allocator> =
        Vec::with_capacity_in(capacity, allocator);

    for entry in auth_list.iter() {
        let AuthorizationEntry {
            chain_id,
            address,
            nonce,
            y_parity,
            r,
            s,
        } = entry;
        let Some((authority, delegated_now)) = validate_authorization::<S>(
            system,
            resources,
            &chain_id,
            nonce,
            address,
            (y_parity, r, s),
            &mut hasher,
        )?
        else {
            system_log!(system, "Authorization skipped\n");
            continue;
        };
        let delegated_before = match seen.iter().find(|(a, _)| *a == authority) {
            Some((_, delegated_before)) => *delegated_before,
            None => {
                seen.push((authority, delegated_now.1));
                delegated_now.1
            }
        };
        if !delegated_now.0 {
            // the authority does not exist: its creation
            resources
                .charge_state_gas(NEW_ACCOUNT)
                .map_err(out_of_gas)?;
        }
        if !paid_writes.contains(&authority) {
            resources
                .charge_legacy_gas(ACCOUNT_WRITE)
                .map_err(out_of_gas)?;
            paid_writes.push(authority);
        }
        let delegation_address = B160::from_be_bytes(*address);
        if delegation_address != B160::ZERO
            && !delegated_before
            && !delegation_set_for.contains(&authority)
        {
            // a delegation indicator the authority did not have: never refilled
            resources.charge_state_gas(AUTH_BASE).map_err(out_of_gas)?;
        }
        if delegation_address != B160::ZERO {
            delegation_set_for.push(authority);
        }
        system_log!(
            system,
            "Will delegate address 0x{:040x} -> 0x{:040x}\n",
            authority.as_uint(),
            delegation_address.as_uint()
        );
        resources.with_infinite_ergs(|inf_ergs| {
            system
                .io
                .set_delegation(inf_ergs, &authority, &delegation_address)
        })?;
        resources
            .with_infinite_ergs(|inf_ergs| {
                system
                    .io
                    .increment_nonce(ExecutionEnvironmentType::NoEE, inf_ergs, &authority, 1)
            })
            .map_err(|e| -> PreparationError {
                match e {
                    SubsystemError::LeafUsage(InterfaceError(NonceError::NonceOverflow, _)) => {
                        PreparationError::Internal(
                            internal_error!("Cannot overflow, already checked").into(),
                        )
                    }
                    _ => PreparationError::Internal(wrap_error!(e)),
                }
            })?;
    }
    Ok(())
}

/// The authority of a valid authorization (chain id, nonce, signature, not a contract,
/// nonce matching), with whether it is alive and whether it is delegated; `None` for an
/// authorization that is skipped. The recovery and the authority's access are in the
/// intrinsic gas.
#[allow(clippy::type_complexity)]
fn validate_authorization<S: EthereumLikeSystem>(
    system: &mut System<S>,
    resources: &mut S::Resources,
    auth_chain_id: &U256,
    auth_nonce: u64,
    delegation_address: &[u8; 20],
    auth_sig_data: (u8, &[u8], &[u8]),
    hasher: &mut crypto::sha3::Keccak256,
) -> Result<Option<(B160, (bool, bool))>, PreparationError>
where
    S::IO: IOSubsystemExt,
    S::Resources: StateGasResources,
{
    let chain_id = system.get_chain_id();
    if !auth_chain_id.is_zero() && auth_chain_id != &U256::from(chain_id) {
        return Ok(None);
    }
    if auth_nonce == u64::MAX {
        return Ok(None);
    }
    let (_, _, auth_s) = auth_sig_data;
    let Some(s) = U256::try_from_be_slice(auth_s) else {
        return Ok(None);
    };
    if s > crypto::secp256k1::SECP256K1N_HALF_U256 {
        return Ok(None);
    }
    let Some(msg) = skipping(resources.with_infinite_ergs(|inf_ergs| {
        compute_auth_message_signed_hash::<S>(
            inf_ergs,
            auth_chain_id,
            auth_nonce,
            delegation_address,
            hasher,
        )
    }))?
    else {
        return Ok(None);
    };
    let Some(Some(authority)) =
        skipping(resources.with_infinite_ergs(|inf_ergs| {
            recover_authority(system, inf_ergs, auth_sig_data, &msg)
        }))?
    else {
        return Ok(None);
    };
    let account_properties = resources.with_infinite_ergs(|inf_ergs| {
        system.io.read_account_properties(
            ExecutionEnvironmentType::NoEE,
            inf_ergs,
            &authority,
            AccountDataRequest::empty()
                .with_nonce()
                .with_nominal_token_balance()
                .with_is_delegated()
                .with_has_bytecode(),
        )
    })?;
    if account_properties.is_contract() {
        return Ok(None);
    }
    if account_properties.nonce.0 != auth_nonce {
        return Ok(None);
    }
    let alive = is_alive(
        account_properties.nonce.0,
        &account_properties.nominal_token_balance.0,
        account_properties.has_bytecode(),
    );
    Ok(Some((
        authority,
        (alive, account_properties.is_delegated.0),
    )))
}

/// A validation failure of an authorization skips it; an internal error is one
fn skipping<T>(r: Result<T, TxError>) -> Result<Option<T>, PreparationError> {
    match r {
        Ok(value) => Ok(Some(value)),
        Err(TxError::Validation(_)) => Ok(None),
        Err(TxError::Internal(e)) => Err(PreparationError::Internal(e)),
    }
}

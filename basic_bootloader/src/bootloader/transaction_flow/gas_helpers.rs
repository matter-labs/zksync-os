use super::super::*;
use crate::bootloader::constants::{
    FRI_PROOF_INTRINSIC_NATIVE_COST_PER_PROOF, FRI_PROOF_TX_INTRINSIC_GAS,
    L1_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE, L1_TX_INTRINSIC_NATIVE_COST,
    L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_ADDRESS,
    L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_STORAGE_KEY,
    L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST,
    L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_AUTHORIZATION,
    L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE, L2_TX_INTRINSIC_PUBDATA,
    L2_TX_INTRINSIC_PUBDATA_PER_AUTHORIZATION, SERVICE_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST,
    SERVICE_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE, TX_INTRINSIC_GAS,
};
use crate::require;
use constants::{CALLDATA_TOKEN_GAS_COST, DEPLOYMENT_TX_EXTRA_INTRINSIC_GAS};
use evm_interpreter::ERGS_PER_GAS;
use zk_ee::out_of_native_resources;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::metadata::basic_metadata::ZkSpecificMetadata;
use zk_ee::system::{Computational, Ergs, Resources};
#[allow(unused_imports)]
use zk_ee::system::{Resource, MAX_NATIVE_COMPUTATIONAL};
use zk_ee::system_log;

pub struct ResourcesForTx<S: EthereumLikeTypes> {
    // Resources to run the transaction.
    // These will be capped to MAX_NATIVE_COMPUTATIONAL, to prevent
    // transaction from using too many native computational resources.
    pub main_resources: S::Resources,
    /// Resources in excess of MAX_NATIVE_COMPUTATIONAL.
    /// These resources can only be used for paying for pubdata.
    pub withheld: S::Resources,
}

impl<S: EthereumLikeTypes> core::fmt::Debug for ResourcesForTx<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ResourcesForTx")
            .field("gas", &(self.main_resources.ergs().0 / ERGS_PER_GAS))
            .field("main_resources", &self.main_resources)
            .field("withheld", &self.withheld)
            .finish()
    }
}

pub fn calculate_l2_tx_intrinsic_computational_native_resources(
    calldata_byte_length: u64,
    access_list_accounts: u64,
    access_list_storages: u64,
    authorization_list_num: u64,
    statement_versioned_hashes_num: u64,
    is_service: bool,
) -> u64 {
    let mut intrinsic_computational_native_resources = if is_service {
        SERVICE_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST
    } else {
        L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST
    };

    intrinsic_computational_native_resources = intrinsic_computational_native_resources
        .saturating_add(calldata_byte_length.saturating_mul(if is_service {
            SERVICE_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE
        } else {
            L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE
        }));

    intrinsic_computational_native_resources = intrinsic_computational_native_resources
        .saturating_add(
            access_list_accounts
                .saturating_mul(L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_ADDRESS),
        );

    intrinsic_computational_native_resources = intrinsic_computational_native_resources
        .saturating_add(
            access_list_storages
                .saturating_mul(L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_STORAGE_KEY),
        );

    intrinsic_computational_native_resources = intrinsic_computational_native_resources
        .saturating_add(
            authorization_list_num
                .saturating_mul(L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_AUTHORIZATION),
        );

    intrinsic_computational_native_resources = intrinsic_computational_native_resources
        .saturating_add(
            statement_versioned_hashes_num
                .saturating_mul(FRI_PROOF_INTRINSIC_NATIVE_COST_PER_PROOF),
        );

    intrinsic_computational_native_resources
}

pub fn calculate_l1_tx_intrinsic_computational_native_resources(calldata_byte_length: u64) -> u64 {
    let mut intrinsic_computational_native_resources = L1_TX_INTRINSIC_NATIVE_COST;

    intrinsic_computational_native_resources = intrinsic_computational_native_resources
        .saturating_add(
            calldata_byte_length
                .saturating_mul(L1_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE),
        );

    intrinsic_computational_native_resources
}

/// Total intrinsic gas for transaction, in EVM gas units.
/// This function used both for L1 and L2 transactions.
///
/// Computes the analogue of revm's `intrinsic_cost`: the gas that must be
/// pre-charged before the transaction body runs.
pub fn calculate_tx_intrinsic_gas(
    calldata_len: u64,
    calldata_tokens: u64,
    is_deployment: bool,
    access_list_accounts: u64,
    access_list_storage_keys: u64,
    authorization_list_num: u64,
    statement_versioned_hashes_num: u64,
) -> u64 {
    let mut intrinsic_gas = TX_INTRINSIC_GAS;

    if is_deployment {
        intrinsic_gas = intrinsic_gas.saturating_add(DEPLOYMENT_TX_EXTRA_INTRINSIC_GAS);
        let initcode_gas_cost =
            evm_interpreter::gas_constants::INITCODE_WORD_COST * calldata_len.div_ceil(32);
        intrinsic_gas = intrinsic_gas.saturating_add(initcode_gas_cost);
    }
    intrinsic_gas =
        intrinsic_gas.saturating_add(calldata_tokens.saturating_mul(CALLDATA_TOKEN_GAS_COST));

    // EIP-2930 access list: per-address + per-storage-key.
    intrinsic_gas = intrinsic_gas.saturating_add(
        access_list_accounts.saturating_mul(evm_interpreter::gas_constants::ACCESS_LIST_ADDRESS),
    );
    intrinsic_gas = intrinsic_gas.saturating_add(
        access_list_storage_keys
            .saturating_mul(evm_interpreter::gas_constants::ACCESS_LIST_STORAGE_KEY),
    );

    // EIP-7702 authorization list: per-authorization. We precharge the
    // empty-account cost; when the authority turns out to be non-empty the
    // delta (NEWACCOUNT - PER_AUTH_BASE_COST) is added back as a gas refund
    // inside `validate_and_apply_delegation`.
    intrinsic_gas = intrinsic_gas.saturating_add(
        authorization_list_num.saturating_mul(evm_interpreter::gas_constants::NEWACCOUNT),
    );

    // FRI statements reserve a user-visible gas surcharge. The verifier cost
    // is also charged via intrinsic native resources.
    intrinsic_gas = intrinsic_gas
        .saturating_add(statement_versioned_hashes_num.saturating_mul(FRI_PROOF_TX_INTRINSIC_GAS));

    intrinsic_gas
}

pub fn calculate_l2_tx_intrinsic_pubdata(authorization_list_num: u64, is_service: bool) -> u64 {
    if is_service {
        // there is no intrinsic pubdata for service txs
        return 0;
    }
    let mut intrinsic_pubdata = L2_TX_INTRINSIC_PUBDATA;

    intrinsic_pubdata = intrinsic_pubdata.saturating_add(
        authorization_list_num.saturating_mul(L2_TX_INTRINSIC_PUBDATA_PER_AUTHORIZATION),
    );

    intrinsic_pubdata
}

///
/// Create initial resources for a transaction. Pure constructor: splits the
/// native budget into `main_resources` (capped at `MAX_NATIVE_COMPUTATIONAL`)
/// and `withheld` (the excess, only spendable on pubdata at refund time), and
/// loads `gas_limit · ERGS_PER_GAS` ergs into `main_resources`.
///
/// Intrinsic gas / native / pubdata are NOT subtracted here. Callers must
/// charge them via [`charge_intrinsic_pubdata`],
/// [`charge_intrinsic_computational_native`] and [`charge_intrinsic_gas`]
/// with whatever error semantics they need (L2 surfaces validation errors,
/// L1 logs and saturates).
///
/// Note: for zero gas price, we use "unlimited native".
pub fn create_resources_for_tx<S: EthereumLikeTypes>(
    gas_limit: u64,
    free_native: bool,
    native_prepaid_from_gas: u64,
) -> ResourcesForTx<S>
where
    S::Metadata: ZkSpecificMetadata,
{
    let native_total = if free_native {
        u64::MAX - 1 // So any saturating subtraction below cannot underflow it
    } else {
        native_prepaid_from_gas
    };

    // Always cap the computational budget at `MAX_NATIVE_COMPUTATIONAL`.
    // Anything above the cap can only be spent on pubdata at refund time.
    let (main_native_u64, withheld) = if native_total <= MAX_NATIVE_COMPUTATIONAL {
        (native_total, S::Resources::from_ergs(Ergs::empty()))
    } else {
        let withheld_native =
            <<S as zk_ee::system::SystemTypes>::Resources as Resources>::Native::from_computational(
                native_total - MAX_NATIVE_COMPUTATIONAL,
            );

        (
            MAX_NATIVE_COMPUTATIONAL,
            S::Resources::from_native(withheld_native),
        )
    };

    let main_native =
        <<S as zk_ee::system::SystemTypes>::Resources as Resources>::Native::from_computational(
            main_native_u64,
        );
    let ergs = gas_limit.saturating_mul(ERGS_PER_GAS);
    let main_resources = S::Resources::from_ergs_and_native(Ergs(ergs), main_native);

    ResourcesForTx {
        main_resources,
        withheld,
    }
}

/// Charge intrinsic pubdata cost (native-only). Drains `withheld` first,
/// then spills into `main_resources`. Underlying `charge` already saturates
/// each resource to zero on insufficient funds; this helper returns `Err(())`
/// if the total budget couldn't cover the cost, so the caller can decide
/// whether to surface a validation error (L2) or just log (L1).
///
/// Equivalent in steady state to the original behavior of
/// `create_resources_for_tx`, which subtracted pubdata cost from the total
/// native budget before splitting into `main`/`withheld`.
pub fn charge_intrinsic_pubdata<S: EthereumLikeTypes>(
    resources: &mut ResourcesForTx<S>,
    intrinsic_pubdata: u64,
    native_per_pubdata: u64,
) -> Result<(), ()> {
    let total_cost = native_per_pubdata.saturating_mul(intrinsic_pubdata);
    if total_cost == 0 {
        return Ok(());
    }

    let withheld_avail = resources.withheld.native().as_u64();
    let from_withheld = total_cost.min(withheld_avail);
    let from_main = total_cost - from_withheld;

    if from_withheld > 0 {
        let cost = S::Resources::from_native(
            <<S::Resources as Resources>::Native as Computational>::from_computational(
                from_withheld,
            ),
        );
        // Saturates withheld to 0 if insufficient — for our `min` choice it
        // should be exact, but be defensive.
        let _ = resources.withheld.charge(&cost);
    }

    if from_main > 0 {
        let cost = S::Resources::from_native(
            <<S::Resources as Resources>::Native as Computational>::from_computational(from_main),
        );
        if resources.main_resources.charge(&cost).is_err() {
            return Err(());
        }
    }

    Ok(())
}

/// Charge intrinsic computational native against `main_resources`.
/// `charge` saturates the resource to zero if insufficient; we surface that
/// as `Err(())` so the caller can map it to its preferred error variant.
pub fn charge_intrinsic_computational_native<S: EthereumLikeTypes>(
    main: &mut S::Resources,
    intrinsic_computational_native: u64,
) -> Result<(), ()> {
    if intrinsic_computational_native == 0 {
        return Ok(());
    }
    let cost = S::Resources::from_native(
        <<S::Resources as Resources>::Native as Computational>::from_computational(
            intrinsic_computational_native,
        ),
    );
    main.charge(&cost).map_err(|_| ())
}

/// Charge intrinsic gas (in EVM gas units) against `main_resources` ergs.
/// `charge` saturates the resource to zero on underflow; we surface that as
/// `Err(())`.
pub fn charge_intrinsic_gas<S: EthereumLikeTypes>(
    main: &mut S::Resources,
    intrinsic_gas: u64,
) -> Result<(), ()> {
    if intrinsic_gas == 0 {
        return Ok(());
    }
    let cost = S::Resources::from_ergs(Ergs(intrinsic_gas.saturating_mul(ERGS_PER_GAS)));
    main.charge(&cost).map_err(|_| ())
}

///
/// Get current pubdata spent and ergs to be charged for it.
/// If base_pubdata is Some, it's discounted from the current
/// pubdata counter.
/// Note: if base_pubdata is greater than the current counter, this function
/// returns 0.
///
pub fn get_resources_to_charge_for_pubdata<S: EthereumLikeTypes>(
    system: &mut System<S>,
    native_per_pubdata: u64,
    base_pubdata: Option<u64>,
) -> Result<(u64, S::Resources), SystemError> {
    let current_pubdata_spent = system
        .net_pubdata_used()?
        .saturating_sub(base_pubdata.unwrap_or(0));
    let native = current_pubdata_spent
        .checked_mul(native_per_pubdata)
        .ok_or(out_of_native_resources!())?;
    let native = <S::Resources as zk_ee::system::Resources>::Native::from_computational(native);
    Ok((current_pubdata_spent, S::Resources::from_native(native)))
}

///
/// Checks if the remaining resources are sufficient to pay for the
/// spent pubdata.
/// If base_pubdata is Some, it's discounted from the current
/// pubdata counter.
/// Returns if the check succeeded, the resources to charge
/// for pubdata and the net pubdata used.
///
pub fn check_enough_resources_for_pubdata<S: EthereumLikeTypes>(
    system: &mut System<S>,
    native_per_pubdata: u64,
    resources: &S::Resources,
    base_pubdata: Option<u64>,
) -> Result<(bool, S::Resources, u64), SystemError> {
    let (pubdata_used, resources_for_pubdata) =
        get_resources_to_charge_for_pubdata(system, native_per_pubdata, base_pubdata)?;
    system_log!(system, "Checking gas for pubdata, resources_for_pubdata: {resources_for_pubdata:?}, resources: {resources:?}\n");
    let enough = resources.has_enough(&resources_for_pubdata);
    Ok((enough, resources_for_pubdata, pubdata_used))
}

///
/// Get the gas price for a transaction.
///
pub(crate) fn get_gas_price<S: EthereumLikeTypes, Config: BasicBootloaderExecutionConfig>(
    system: &mut System<S>,
    max_fee_per_gas: &U256,
    max_priority_fee_per_gas: Option<&U256>,
) -> Result<U256, TxError> {
    let base_fee = system.get_eip1559_basefee();
    // If base fee is zero, then we ignore priority fee
    // TODO: not ignore?
    if base_fee.is_zero() {
        Ok(U256::ZERO)
    } else {
        let max_priority_fee_per_gas = max_priority_fee_per_gas.unwrap_or(max_fee_per_gas);
        require!(
            max_priority_fee_per_gas <= max_fee_per_gas,
            TxError::Validation(InvalidTransaction::PriorityFeeGreaterThanMaxFee,),
            system
        )?;
        if !Config::SIMULATION {
            // Skip this check on simulation
            require!(
                &base_fee <= max_fee_per_gas,
                TxError::Validation(InvalidTransaction::BaseFeeGreaterThanMaxFee,),
                system
            )?;
        }
        let priority_fee_per_gas =
            (*max_priority_fee_per_gas).min(max_fee_per_gas.saturating_sub(base_fee));
        // Normally, max_fee_per_gas >= base_fee + priority_fee_per_gas,
        // but we add this min to make it work in simulation too, where we do not
        // enforce max_fee_per_gas > base_fee.
        let gas_price = (base_fee.saturating_add(priority_fee_per_gas)).min(*max_fee_per_gas);
        Ok(gas_price)
    }
}

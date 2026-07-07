use super::super::*;
use crate::bootloader::constants::{
    FRI_PROOF_INTRINSIC_NATIVE_COST_PER_PROOF, FRI_PROOF_TX_INTRINSIC_GAS,
    L1_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE, L1_TX_INTRINSIC_NATIVE_COST,
    L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_ADDRESS,
    L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_STORAGE_KEY,
    L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST, L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST_FREE,
    L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_AUTHORIZATION,
    L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_BLOB_VERSIONED_HASH,
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

/// Per-transaction quantities that drive the intrinsic computational-native
/// cost. Grouping them into a struct avoids an error-prone positional argument
/// list of same-typed `u64`/`bool` values (in particular the adjacent blob- and
/// statement-versioned-hash counts, which would silently transpose).
#[derive(Clone, Copy, Debug, Default)]
pub struct L2TxIntrinsicNativeInput {
    pub calldata_byte_length: u64,
    pub access_list_accounts: u64,
    pub access_list_storages: u64,
    pub authorization_list_num: u64,
    pub blob_versioned_hashes_num: u64,
    pub statement_versioned_hashes_num: u64,
    pub is_service: bool,
    pub free_native: bool,
}

pub fn calculate_l2_tx_intrinsic_computational_native_resources(
    input: &L2TxIntrinsicNativeInput,
) -> u64 {
    let L2TxIntrinsicNativeInput {
        calldata_byte_length,
        access_list_accounts,
        access_list_storages,
        authorization_list_num,
        blob_versioned_hashes_num,
        statement_versioned_hashes_num,
        is_service,
        free_native,
    } = *input;

    let mut intrinsic_computational_native_resources = if is_service {
        SERVICE_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST
    } else if free_native {
        L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST_FREE
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
            blob_versioned_hashes_num
                .saturating_mul(L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_BLOB_VERSIONED_HASH),
        );

    intrinsic_computational_native_resources = intrinsic_computational_native_resources
        .saturating_add(
            statement_versioned_hashes_num
                .saturating_mul(FRI_PROOF_INTRINSIC_NATIVE_COST_PER_PROOF),
        );

    intrinsic_computational_native_resources
}

#[cfg(test)]
mod tests {
    use super::{
        calculate_l2_tx_intrinsic_computational_native_resources, L2TxIntrinsicNativeInput,
    };
    use crate::bootloader::constants::L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_BLOB_VERSIONED_HASH;

    #[test]
    fn l2_intrinsic_native_accounts_for_blob_versioned_hashes() {
        let without_blobs = calculate_l2_tx_intrinsic_computational_native_resources(
            &L2TxIntrinsicNativeInput::default(),
        );
        let with_blobs =
            calculate_l2_tx_intrinsic_computational_native_resources(&L2TxIntrinsicNativeInput {
                blob_versioned_hashes_num: 6,
                ..Default::default()
            });

        assert_eq!(
            with_blobs - without_blobs,
            6 * L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_BLOB_VERSIONED_HASH
        );
    }
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
/// Create initial resources for a transaction, applying all three intrinsic
/// charges (pubdata, computational native, gas) in u64 arithmetic.
///
/// Charges are saturating: on underflow the running counter goes to 0, the
/// first observed failure is recorded, and remaining charges still apply.
///
pub fn create_resources_for_tx<S: EthereumLikeTypes>(
    gas_limit: u64,
    free_native: bool,
    native_prepaid_from_gas: u64,
    native_per_pubdata_byte: u64,
    intrinsic_gas: u64,
    intrinsic_computational_native: u64,
    intrinsic_pubdata: u64,
) -> (ResourcesForTx<S>, Option<InvalidTransaction>)
where
    S::Metadata: ZkSpecificMetadata,
{
    let mut first_error: Option<InvalidTransaction> = None;

    // Gross native budget.
    let native_limit = if free_native {
        u64::MAX - 1 // So any saturating subtraction below cannot underflow it
    } else {
        native_prepaid_from_gas
    };

    // Charge intrinsic pubdata. Subtracted from the total native budget
    // before splitting into main / withheld.
    let intrinsic_pubdata_overhead = native_per_pubdata_byte.saturating_mul(intrinsic_pubdata);
    let native_limit = match native_limit.checked_sub(intrinsic_pubdata_overhead) {
        Some(val) => val,
        None => {
            first_error.get_or_insert(InvalidTransaction::OutOfNativeResourcesDuringValidation);
            0
        }
    };

    // Split: anything above MAX_NATIVE_COMPUTATIONAL goes into `withheld`
    // (only spendable on pubdata at refund time).
    let (native_limit, withheld) = if native_limit <= MAX_NATIVE_COMPUTATIONAL {
        (native_limit, S::Resources::from_ergs(Ergs::empty()))
    } else {
        let withheld_native =
            <<S as zk_ee::system::SystemTypes>::Resources as Resources>::Native::from_computational(
                native_limit - MAX_NATIVE_COMPUTATIONAL,
            );
        (
            MAX_NATIVE_COMPUTATIONAL,
            S::Resources::from_native(withheld_native),
        )
    };

    // Charge intrinsic computational native against the post-split main budget.
    let native_limit = match native_limit.checked_sub(intrinsic_computational_native) {
        Some(val) => val,
        None => {
            first_error.get_or_insert(InvalidTransaction::OutOfNativeResourcesDuringValidation);
            0
        }
    };
    let native_limit =
        <<S as zk_ee::system::SystemTypes>::Resources as Resources>::Native::from_computational(
            native_limit,
        );

    // Charge intrinsic gas against gas_limit.
    let gas_limit_for_tx = match gas_limit.checked_sub(intrinsic_gas) {
        Some(val) => val,
        None => {
            first_error.get_or_insert(InvalidTransaction::OutOfGasDuringValidation);
            0
        }
    };
    let ergs = gas_limit_for_tx.saturating_mul(ERGS_PER_GAS);

    let main_resources = S::Resources::from_ergs_and_native(Ergs(ergs), native_limit);
    (
        ResourcesForTx {
            main_resources,
            withheld,
        },
        first_error,
    )
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

use super::super::*;
use crate::bootloader::constants::{L1_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE, L1_TX_INTRINSIC_NATIVE_COST, L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_ADDRESS, L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_STORAGE_KEY, L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST, L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_AUTHORIZATION, L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE, L2_TX_INTRINSIC_PUBDATA, L2_TX_INTRINSIC_PUBDATA_PER_AUTHORIZATION, SERVICE_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST, SERVICE_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE, TX_INTRINSIC_GAS};
use crate::require;
use constants::{CALLDATA_TOKEN_GAS_COST, DEPLOYMENT_TX_EXTRA_INTRINSIC_GAS};
use evm_interpreter::ERGS_PER_GAS;
use zk_ee::out_of_native_resources;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::metadata::basic_metadata::ZkSpecificPricingMetadata;
use zk_ee::system::{Computational, Ergs, Resources};
#[allow(unused_imports)]
use zk_ee::system::{Resource, MAX_NATIVE_COMPUTATIONAL};
use zk_ee::system_log;

/// Policy trait for handling arithmetic validation errors during resource creation.
///
/// This trait allows L1 and L2 transactions to handle errors differently:
/// - L1: Only returns internal errors (validation errors are logged and saturated)
/// - L2: Returns both validation and internal errors
pub trait ResourcesCreationErrorPolicy<S: EthereumLikeTypes> {
    /// The return error type for create_resources_for_tx.
    /// For L1: BootloaderSubsystemError (no validation errors possible)
    /// For L2: TxError (both validation and internal errors)
    type Error;

    /// The error type that describes arithmetic validation failures.
    /// For L1: a descriptive enum for logging
    /// For L2: InvalidTransaction
    type ArithmeticError;

    /// Create an error for native limit underflow
    fn native_underflow_error(operation: &'static str) -> Self::ArithmeticError;

    /// Create an error for intrinsic gas exceeding gas limit
    fn intrinsic_gas_overflow_error(
        intrinsic_overhead: u64,
        gas_limit: u64,
    ) -> Self::ArithmeticError;

    /// Handle an arithmetic validation error.
    /// For L1: logs the error and returns Ok(saturated_value)
    /// For L2: returns Err(Self::Error)
    fn handle_arithmetic_error(
        system: &mut System<S>,
        error: Self::ArithmeticError,
    ) -> Result<u64, Self::Error>;

    /// Convert an internal error to the policy's error type
    #[allow(dead_code)] // Reserved for future use if internal errors are added
    fn from_internal_error(error: BootloaderSubsystemError) -> Self::Error;

    /// Convert a validation error to the policy's error type.
    /// For L1: should never be called (deployment checks don't apply)
    /// For L2: wraps in TxError::Validation
    fn from_validation_error(error: InvalidTransaction) -> Self::Error;
}

/// Arithmetic error descriptor for L1 transactions
#[derive(Debug)]
pub enum L1ArithmeticError {
    /// Native limit underflow during an operation
    NativeUnderflow { operation: &'static str },
    /// Gas limit is less than intrinsic gas overhead
    IntrinsicGasOverflow {
        intrinsic_overhead: u64,
        gas_limit: u64,
    },
}

/// Resource creation policy for L1 transactions: log and saturate on errors
pub struct L1ResourcesPolicy;

impl<S: EthereumLikeTypes> ResourcesCreationErrorPolicy<S> for L1ResourcesPolicy {
    type Error = BootloaderSubsystemError;
    type ArithmeticError = L1ArithmeticError;

    fn native_underflow_error(operation: &'static str) -> Self::ArithmeticError {
        L1ArithmeticError::NativeUnderflow { operation }
    }

    fn intrinsic_gas_overflow_error(
        intrinsic_overhead: u64,
        gas_limit: u64,
    ) -> Self::ArithmeticError {
        L1ArithmeticError::IntrinsicGasOverflow {
            intrinsic_overhead,
            gas_limit,
        }
    }

    fn handle_arithmetic_error(
        system: &mut System<S>,
        error: Self::ArithmeticError,
    ) -> Result<u64, Self::Error> {
        match error {
            L1ArithmeticError::NativeUnderflow { operation } => {
                system_log!(
                    system,
                    "Native underflow during {}, saturating to 0 for L1 tx",
                    operation
                );
                Ok(0)
            }
            L1ArithmeticError::IntrinsicGasOverflow {
                intrinsic_overhead,
                gas_limit,
            } => {
                system_log!(
                    system,
                    "Gas limit {} < intrinsic gas {} for L1 tx, saturating to 0",
                    gas_limit,
                    intrinsic_overhead
                );
                Ok(0)
            }
        }
    }

    fn from_internal_error(error: BootloaderSubsystemError) -> Self::Error {
        error
    }

    fn from_validation_error(error: InvalidTransaction) -> Self::Error {
        // L1 transactions never have deployment validation, so this should never be called
        unreachable!(
            "L1ResourcesPolicy should never encounter validation error: {:?}",
            error
        )
    }
}

/// Resource creation policy for L2 transactions: fail on arithmetic errors
pub struct L2ResourcesPolicy;

impl<S: EthereumLikeTypes> ResourcesCreationErrorPolicy<S> for L2ResourcesPolicy {
    type Error = TxError;
    type ArithmeticError = InvalidTransaction;

    fn native_underflow_error(_operation: &'static str) -> Self::ArithmeticError {
        InvalidTransaction::OutOfNativeResourcesDuringValidation
    }

    fn intrinsic_gas_overflow_error(
        _intrinsic_overhead: u64,
        _gas_limit: u64,
    ) -> Self::ArithmeticError {
        InvalidTransaction::OutOfGasDuringValidation
    }

    fn handle_arithmetic_error(
        _system: &mut System<S>,
        error: Self::ArithmeticError,
    ) -> Result<u64, Self::Error> {
        Err(TxError::Validation(error))
    }

    fn from_internal_error(error: BootloaderSubsystemError) -> Self::Error {
        TxError::Internal(error)
    }

    fn from_validation_error(error: InvalidTransaction) -> Self::Error {
        TxError::Validation(error)
    }
}

pub struct ResourcesForTx<S: EthereumLikeTypes> {
    // Resources to run the transaction.
    // These will be capped to MAX_NATIVE_COMPUTATIONAL, to prevent
    // transaction from using too many native computational resources.
    pub main_resources: S::Resources,
    /// Resources in excess of MAX_NATIVE_COMPUTATIONAL.
    /// These resources can only be used for paying for pubdata.
    pub withheld: S::Resources,
    /// Computational native charged for as intrinsic
    pub intrinsic_computational_native_charged: u64,
}

impl<S: EthereumLikeTypes> core::fmt::Debug for ResourcesForTx<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ResourcesForTx")
            .field("gas", &(self.main_resources.ergs().0 / ERGS_PER_GAS))
            .field("main_resources", &self.main_resources)
            .field("withheld", &self.withheld)
            .field(
                "intrinsic_computational_native_charged",
                &self.intrinsic_computational_native_charged,
            )
            .finish()
    }
}

/// Initial native budget for the `verify_intrinsic_native` tracker.
///
/// Must be strictly greater than every possible `formula_value` the tracker
/// is ever compared against, so that the tracker never exhausts during a real
/// transaction. We pick a value that far exceeds `MAX_NATIVE_COMPUTATIONAL`
/// so saturation in the underlying DecreasingNative cannot happen in practice.
#[cfg(feature = "verify_intrinsic_native")]
pub const INTRINSIC_TRACKER_INITIAL_NATIVE: u64 = 1u64 << 50;

/// Build the resources value used by the `verify_intrinsic_native` tracker.
///
/// In production (feature off) this is `FORMAL_INFINITE`, so the behavior of
/// code that currently uses `S::Resources::FORMAL_INFINITE` is unchanged.
/// With the feature on, ergs remain effectively infinite (so no call site
/// unexpectedly OOGs) but the native component starts at a finite, known
/// value so the actual native consumption can be recovered from the residual.
pub fn make_intrinsic_tracker<S: EthereumLikeTypes>() -> S::Resources {
    #[cfg(feature = "verify_intrinsic_native")]
    {
        S::Resources::from_ergs_and_native(
            Ergs(u64::MAX),
            <<S as zk_ee::system::SystemTypes>::Resources as Resources>::Native::from_computational(
                INTRINSIC_TRACKER_INITIAL_NATIVE,
            ),
        )
    }
    #[cfg(not(feature = "verify_intrinsic_native"))]
    {
        S::Resources::FORMAL_INFINITE
    }
}

pub fn calculate_l2_tx_intrinsic_computational_native_resources(
    calldata_byte_length: u64,
    access_list_accounts: u64,
    access_list_storages: u64,
    authorization_list_num: u64,
    is_service: bool,
) -> u64 {
    let mut intrinsic_computational_native_resources = if is_service { SERVICE_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST } else { L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST };

    intrinsic_computational_native_resources = intrinsic_computational_native_resources
        .saturating_add(
            calldata_byte_length
                .saturating_mul( if is_service { SERVICE_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE } else { L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE}),
        );

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
/// pre-charged before the transaction body runs. Per EIP-2930/EIP-7702 the
/// per-address, per-storage-key and per-authorization costs are part of this
/// intrinsic gas. Moving them into this helper means the inner access-list /
/// authorization-list processors only need to account for native resources —
/// gas is already deducted from `main_resources` when the tx's resources are
/// materialized.
pub fn calculate_tx_intrinsic_gas(
    calldata_len: u64,
    calldata_tokens: u64,
    is_deployment: bool,
    access_list_accounts: u64,
    access_list_storage_keys: u64,
    authorization_list_num: u64,
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
/// Create initial resources for a transaction.
///
/// The `P` parameter controls how arithmetic validation errors are handled:
/// - Use `L1ResourcesPolicy` for L1 transactions: logs and saturates (never fails validation)
///   Returns `Result<..., BootloaderSubsystemError>` - validation errors are impossible
/// - Use `L2ResourcesPolicy` for L2 transactions: returns validation errors
///   Returns `Result<..., TxError>` - can fail with validation or internal errors
pub fn create_resources_for_tx<S: EthereumLikeTypes, P: ResourcesCreationErrorPolicy<S>>(
    system: &mut System<S>,
    gas_limit: u64,
    free_native: bool,
    native_prepaid_from_gas: u64,
    native_per_pubdata_byte: u64,
    intrinsic_gas: u64,
    intrinsic_computational_native: u64,
    intrinsic_pubdata: u64,
) -> Result<ResourcesForTx<S>, P::Error>
where
    S::Metadata: ZkSpecificPricingMetadata,
{
    // This is the real limit, which we later use to compute native_used.
    // From it, we discount intrinsic pubdata and then take the min
    // with the MAX_NATIVE_COMPUTATIONAL.
    // We do those operations in that order because the pubdata charge
    // isn't computational.
    // We can consider in the future to keep two limits, so that pubdata
    // is not charged from computational resource.
    // Note: for zero gas price, we use "unlimited native"
    let native_limit = if cfg!(feature = "unlimited_native") || free_native {
        u64::MAX - 1 // So any saturation below can not be subtracted from it
    } else {
        native_prepaid_from_gas
    };

    // Charge intrinsic pubdata
    let intrinsic_pubdata_overhead = native_per_pubdata_byte.saturating_mul(intrinsic_pubdata);
    let native_limit = match native_limit.checked_sub(intrinsic_pubdata_overhead) {
        Some(val) => val,
        None => P::handle_arithmetic_error(
            system,
            P::native_underflow_error("subtracting pubdata overhead"),
        )?,
    };

    // EVM tester requires high native limits, so for it we never hold off resources.
    // But for the real world, we bound the available resources.

    #[cfg(feature = "resources_for_tester")]
    let withheld = S::Resources::from_ergs(Ergs::empty());

    #[cfg(not(feature = "resources_for_tester"))]
    let (native_limit, withheld) = if native_limit <= MAX_NATIVE_COMPUTATIONAL {
        (native_limit, S::Resources::from_ergs(Ergs::empty()))
    } else {
        let withheld =
            <<S as zk_ee::system::SystemTypes>::Resources as Resources>::Native::from_computational(
                native_limit - MAX_NATIVE_COMPUTATIONAL,
            );

        (
            MAX_NATIVE_COMPUTATIONAL,
            S::Resources::from_native(withheld),
        )
    };

    // Charge intrinsic computational native
    let native_limit = match native_limit.checked_sub(intrinsic_computational_native) {
        Some(val) => val,
        None => P::handle_arithmetic_error(
            system,
            P::native_underflow_error("subtracting intrinsic computational native"),
        )?,
    };

    let native_limit =
        <<S as zk_ee::system::SystemTypes>::Resources as Resources>::Native::from_computational(
            native_limit,
        );

    // Check if intrinsic gas exceeds gas limit
    let gas_limit_for_tx = match gas_limit.checked_sub(intrinsic_gas) {
        Some(val) => val,
        None => P::handle_arithmetic_error(
            system,
            P::intrinsic_gas_overflow_error(intrinsic_gas, gas_limit),
        )?,
    };

    let ergs = gas_limit_for_tx.saturating_mul(ERGS_PER_GAS); // we checked at the very start that gas_limit * ERGS_PER_GAS doesn't overflow
    let main_resources = S::Resources::from_ergs_and_native(Ergs(ergs), native_limit);

    Ok(ResourcesForTx {
        main_resources,
        withheld,
        intrinsic_computational_native_charged: intrinsic_computational_native,
    })
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

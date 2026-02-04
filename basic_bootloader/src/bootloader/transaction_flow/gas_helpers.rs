use crate::require;
use constants::{CALLDATA_TOKEN_GAS_COST, DEPLOYMENT_TX_EXTRA_INTRINSIC_GAS};
use evm_interpreter::{ERGS_PER_GAS, MAX_INITCODE_SIZE};
use zk_ee::out_of_native_resources;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::metadata::basic_metadata::ZkSpecificPricingMetadata;
use zk_ee::system::{Computational, Ergs, Resources};
#[allow(unused_imports)]
use zk_ee::system::{Resource, MAX_NATIVE_COMPUTATIONAL};
use zk_ee::system_log;

use super::super::*;

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
    is_deployment: bool,
    calldata_len: u64,
    calldata_tokens: u64,
    intrinsic_gas: u64,
    intrinsic_pubdata: u64,
    intrinsic_native: u64,
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

    // Charge pubdata overhead
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

    // Charge for calldata and intrinsic native
    let calldata_native = calldata_len
        .saturating_mul(evm_interpreter::native_resource_constants::COPY_BYTE_NATIVE_COST);
    let intrinsic_computational_native_charged = calldata_native.saturating_add(intrinsic_native);

    let native_limit = match native_limit.checked_sub(intrinsic_computational_native_charged) {
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

    // Intrinsic overhead - he can quickly check deployment cost and calldata tokens cost
    let mut intrinsic_overhead = intrinsic_gas;

    if is_deployment {
        if calldata_len > MAX_INITCODE_SIZE as u64 {
            return Err(P::from_validation_error(
                InvalidTransaction::CreateInitCodeSizeLimit,
            ));
        }
        intrinsic_overhead = intrinsic_overhead.saturating_add(DEPLOYMENT_TX_EXTRA_INTRINSIC_GAS);
        let initcode_gas_cost =
            evm_interpreter::gas_constants::INITCODE_WORD_COST * calldata_len.div_ceil(32);
        intrinsic_overhead = intrinsic_overhead.saturating_add(initcode_gas_cost);
    }
    intrinsic_overhead =
        intrinsic_overhead.saturating_add(calldata_tokens.saturating_mul(CALLDATA_TOKEN_GAS_COST));

    // Check if intrinsic gas exceeds gas limit
    let gas_limit_for_tx = match gas_limit.checked_sub(intrinsic_overhead) {
        Some(val) => val,
        None => P::handle_arithmetic_error(
            system,
            P::intrinsic_gas_overflow_error(intrinsic_overhead, gas_limit),
        )?,
    };

    let ergs = gas_limit_for_tx.saturating_mul(ERGS_PER_GAS); // we checked at the very start that gas_limit * ERGS_PER_GAS doesn't overflow
    let main_resources = S::Resources::from_ergs_and_native(Ergs(ergs), native_limit);

    Ok(ResourcesForTx {
        main_resources,
        withheld,
        intrinsic_computational_native_charged,
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

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
use crate::bootloader::constants::{L1_TX_INTRINSIC_NATIVE_COST, TX_INTRINSIC_GAS};
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

pub fn calculate_l2_tx_intrinsic_computational_native_resources(
    calldata_byte_length: u64,
    access_list_accounts: u64,
    access_list_storages: u64,
    authorization_list_num: u64,
) -> u64 {
    // TODO: include preimage + blake in the account read cost?

    // 1. 30_000 for post validation processing: transferring fee to coinbase, transferring the gas refund, hashing of tx hash into rolling hash.
    // 2. 522000 = 350_000 + 43_000*4 for ecrecover.
    // 3. account read(worst case): 252220 = 500(PREIMAGE_CACHE_GET_NATIVE_COST) + 800 + 340 * 2(blake2s_native_cost(124)) + 4000(WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST) + 4000(WARM_STORAGE_READ_NATIVE_COST) + 242240(COLD_NEW_STORAGE_READ_NATIVE_COST)
    // 4. nonce write: 5000 = 4000(WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST) + 1000(WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST)
    // 5. keccak for signing hash(worst case const part - 2 rounds + 1 round precharge for dynamic parts): 37500 = 2500 + 17_500 * 3
    // 6. keccak for full hash(worst case const part - 2 rounds + 1 round precharge for calldata): 37500 = 2500 + 17_500 * 3
    // 7. fee prepayment: 5000 = 4000(WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST) + 1000(WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST)
    // NOTE: we are precharging 1 keccak round(in 5 and 6) since dynamic part, can consume 136*n + 1 bytes in encoding, so it will pay for ~n rounds, but consume (n + 1) rounds of keccak
    const INTRINSIC_COMPUTATIONAL_NATIVE_CONST: u64 = 30_000 + 522_000 + 252_220 + 5_000 + 55_000 + 55_000 + 5_000;

    // 1. caldata per byte copy: COPY_BYTE_NATIVE_COST = 1
    // 2. keccak for signing hash: 17_500 div_ceil 136 = 129
    // 3. keccak for full hash: 17_500 div_ceil 136 = 129
    const INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE: u64 = 1 + 129 + 129;

    // 1. computational part: 2000
    // 2. account read(worst case): 252220 = 500(PREIMAGE_CACHE_GET_NATIVE_COST) + 800 + 340 * 2(blake2s_native_cost(124)) + 4000(WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST) + 4000(WARM_STORAGE_READ_NATIVE_COST) + 242240(COLD_NEW_STORAGE_READ_NATIVE_COST)
    // 3. keccak for signing hash: 30(worst case contribution to rlp encoding, 21 address, 9 keys list length encoding) * 17500 / 136 = 3861
    // 4. keccak for full hash: 30(worst case contribution to rlp encoding, 21 address, 9 keys list length encoding) * 17500 / 136 = 3861
    const INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_ADDRESS: u64 = 2_000 + 252_220 + 3_861 + 3_861;

    // 1. computational part: 2000
    // 2. storage slot read(worst case): 242240 (COLD_NEW_STORAGE_READ_NATIVE_COST)
    // 3. keccak for signing hash: 33(contribution to rlp encoding length) * 17500 / 136 = 4247
    // 4. keccak for full hash: 33(contribution to rlp encoding length) * 17500 / 136 = 4247
    const INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_STORAGE_KEY: u64 = 2_000 + 242_240 + 4_247 + 4_247;

    // 1. computational part: 2000
    // 2. auth message keccak cost: 2500 + 17_500 = 20_000 (length 70, 1 round)
    // 3. 522000 = 350_000 + 43_000*4 for ecrecover.
    // 4. account read(worst case): 252220 = 500(PREIMAGE_CACHE_GET_NATIVE_COST) + 800 + 340 * 2(blake2s_native_cost(124)) + 4000(WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST) + 4000(WARM_STORAGE_READ_NATIVE_COST) + 242240(COLD_NEW_STORAGE_READ_NATIVE_COST)
    // 5. nonce write: 5000 = 4000(WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST) + 1000(WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST)
    // 6. delegation write: 26640 = 4000(WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST) + 1000(WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST) + 500(PREIMAGE_CACHE_SET_NATIVE_COST) + 20_000(1 round keccak, 23 byte code) + 1140(1 round blake, 24 byte padded code)
    const INTRINSIC_COMPUTATIONAL_NATIVE_PER_AUTHORIZATION: u64 = 2_000 + 20_000 + 522_000 + 252_220 + 5_000 + 26_640;

    let mut intrinsic_computational_native_resources = INTRINSIC_COMPUTATIONAL_NATIVE_CONST;

    intrinsic_computational_native_resources = intrinsic_computational_native_resources.saturating_add(
        calldata_byte_length.saturating_mul(
            INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE
        )
    );

    intrinsic_computational_native_resources = intrinsic_computational_native_resources.saturating_add(
        access_list_accounts.saturating_mul(
            INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_ADDRESS
        )
    );

    intrinsic_computational_native_resources = intrinsic_computational_native_resources.saturating_add(
        access_list_storages.saturating_mul(
            INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_STORAGE_KEY
        )
    );

    intrinsic_computational_native_resources = intrinsic_computational_native_resources.saturating_add(
        authorization_list_num.saturating_mul(
            INTRINSIC_COMPUTATIONAL_NATIVE_PER_AUTHORIZATION
        )
    );

    intrinsic_computational_native_resources
}


pub fn calculate_l1_tx_intrinsic_computational_native_resources(
    calldata_byte_length: u64,
) -> u64 {
    let mut intrinsic_computational_native_resources = L1_TX_INTRINSIC_NATIVE_COST;

    intrinsic_computational_native_resources = intrinsic_computational_native_resources.saturating_add(
        calldata_byte_length.saturating_mul(
            evm_interpreter::native_resource_constants::COPY_BYTE_NATIVE_COST
        )
    );

    intrinsic_computational_native_resources
}

pub fn calculate_l2_tx_intrinsic_pubdata(
    authorization_list_num: u64,
) -> u64 {
    // 1. sender account change: 68 = 32(key) + 1(account metadata) + 2(nonce increase) + 33(worst case balance)
    // 2. coinbase: 66 = 32(key) + 1(account metadata) + 33(worst case balance)
    const INTRINSIC_PUBDATA_CONST: u64 = 68 + 64;


    // Full diff compression:
    // 1. key: 32
    // 2. account metadata: 1
    // 3. versioning data: 8
    // 4. nonce: 2
    // 5. balance: 1
    // 6. unpadded code length: 4
    // 7. artifacts length: 4
    // 8. padded bytecode: 24
    // 9. observable length: 4
    const INTRINSIC_PUBDATA_PER_AUTHORIZATION: u64 = 32 + 1 + 8 + 2 + 1 + 4 + 4 + 24 + 4;

    let mut intrinsic_pubdata = INTRINSIC_PUBDATA_CONST;

    intrinsic_pubdata = intrinsic_pubdata.saturating_add(
        authorization_list_num.saturating_mul(
            INTRINSIC_PUBDATA_PER_AUTHORIZATION
        )
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
    is_deployment: bool,
    calldata_len: u64,
    calldata_tokens: u64,
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

    // Intrinsic gas overhead - he can quickly check deployment cost and calldata tokens cost
    let mut intrinsic_overhead = TX_INTRINSIC_GAS;

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
    // TODO: intrinsic for access list? authorization list?

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

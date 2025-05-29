use constants::{CALLDATA_NON_ZERO_BYTE_GAS_COST, CALLDATA_ZERO_BYTE_GAS_COST};
use evm_interpreter::native_resource_constants::COPY_BYTE_NATIVE_COST;
use evm_interpreter::ERGS_PER_GAS;
use zk_ee::system::errors::InternalError;
use zk_ee::system::{Computational, Resources};

use super::*;

pub fn get_resources_for_tx<S: EthereumLikeTypes>(
    gas_limit: u64,
    native_per_pubdata: U256,
    native_per_gas: U256,
    calldata: &[u8],
    intrinsic_gas: usize,
    intrinsic_pubdata: usize,
    intrinsic_native: usize,
) -> Result<S::Resources, TxError> {
    // TODO: operator trusted gas limit?

    // This is the real limit, which we later use to compute native_used.
    // From it, we discount intrinsic pubdata and then take the min
    // with the MAX_NATIVE_COMPUTATIONAL.
    // We do those operations in that order because the pubdata charge
    // isn't computational.
    // We can consider in the future to keep two limits, so that pubdata
    // is not charged from computational resource.
    let native_limit = gas_limit.saturating_mul(u256_to_u64_saturated(&native_per_gas));

    // Charge pubdata overhead
    let intrinsic_pubdata_overhead = u256_to_u64_saturated(&native_per_pubdata)
        .checked_mul(intrinsic_pubdata as u64)
        .ok_or(InternalError("npp*ip"))?;
    let native_limit =
        native_limit
            .checked_sub(intrinsic_pubdata_overhead)
            .ok_or(TxError::Validation(
                errors::InvalidTransaction::AAValidationError(
                    errors::InvalidAA::OutOfNativeResourcesDuringValidation,
                ),
            ))?;

    // EVM tester requires high native limits
    #[cfg(not(feature = "resources_for_tester"))]
    let native_limit = native_limit.min(MAX_NATIVE_COMPUTATIONAL);

    // Charge for calldata and intrinsic native
    let (calldata_gas, calldata_native) = cost_for_calldata(calldata)?;

    let native_limit = native_limit
        .checked_sub(calldata_native)
        .and_then(|native| native.checked_sub(intrinsic_native as u64))
        .ok_or(TxError::Validation(
            errors::InvalidTransaction::AAValidationError(
                errors::InvalidAA::OutOfNativeResourcesDuringValidation,
            ),
        ))?;

    let native_limit =
        <<S as zk_ee::system::SystemTypes>::Resources as Resources>::Native::from_computational(
            native_limit,
        );

    // Intrinsic overhead
    let intrinsic_overhead = intrinsic_gas as u64;

    let total_gas_to_charge = (calldata_gas as u64)
        .checked_add(intrinsic_overhead)
        .ok_or(InternalError("tuo+io"))?;

    if total_gas_to_charge > gas_limit {
        Err(TxError::Validation(
            errors::InvalidTransaction::AAValidationError(
                errors::InvalidAA::OutOfGasDuringValidation,
            ),
        ))
    } else {
        let gas_limit_for_tx = gas_limit - total_gas_to_charge;
        let ergs = gas_limit_for_tx
            .checked_mul(ERGS_PER_GAS)
            .ok_or(InternalError("glft*EPF"))?;
        let mut resources = S::Resources::from_ergs_and_native(Ergs(ergs), native_limit);
        resources.set_as_limit();
        Ok(resources)
    }
}
///
/// Computes the (gas, native) cost for the transaction's calldata.
///
pub fn cost_for_calldata(calldata: &[u8]) -> Result<(usize, u64), InternalError> {
    let zero_bytes = calldata.iter().filter(|byte| **byte == 0).count();
    let non_zero_bytes = calldata.len() - zero_bytes;
    let zero_cost = zero_bytes
        .checked_mul(CALLDATA_ZERO_BYTE_GAS_COST)
        .ok_or(InternalError("zb*CZBGC"))?;
    let non_zero_cost = non_zero_bytes
        .checked_mul(CALLDATA_NON_ZERO_BYTE_GAS_COST)
        .ok_or(InternalError("nzb*CNZBGC"))?;
    let gas_cost = zero_cost
        .checked_add(non_zero_cost)
        .ok_or(InternalError("zc+nzc"))?;
    let native_cost = (calldata.len() as u64)
        .checked_mul(COPY_BYTE_NATIVE_COST)
        .ok_or(InternalError("cl*CBNC"))?;
    Ok((gas_cost, native_cost))
}

///
/// Get current pubdata spent and ergs to be charged for it.
/// If base_pubdata is Some, it's discounted from the current
/// pubdata counter.
///
pub fn get_resources_to_charge_for_pubdata<S: EthereumLikeTypes>(
    system: &mut System<S>,
    native_per_pubdata: U256,
    base_pubdata: Option<u64>,
) -> Result<(u64, S::Resources), InternalError> {
    let current_pubdata_spent = system.net_pubdata_used() - base_pubdata.unwrap_or(0);
    let native_per_pubdata = u256_to_u64_saturated(&native_per_pubdata);
    let native = current_pubdata_spent
        .checked_mul(native_per_pubdata)
        .ok_or(InternalError("cps*epp"))?;
    let native = <S::Resources as zk_ee::system::Resources>::Native::from_computational(native);
    Ok((current_pubdata_spent, S::Resources::from_native(native)))
}

///
/// Checks if the remaining resources are sufficient to pay for the
/// spent pubdata.
/// If base_pubdata is Some, it's discounted from the current
/// pubdata counter.
/// Returns if the check succeeded.
///
pub fn check_enough_resources_for_pubdata<S: EthereumLikeTypes>(
    system: &mut System<S>,
    native_per_pubdata: U256,
    resources: &S::Resources,
    base_pubdata: Option<u64>,
) -> Result<bool, InternalError> {
    let (_, resources_for_pubdata) =
        get_resources_to_charge_for_pubdata(system, native_per_pubdata, base_pubdata)?;
    let _ = system.get_logger().write_fmt(format_args!(
        "Checking gas for pubdata, resources_for_pubdata: {:?}, resources: {:?}\n",
        resources_for_pubdata, resources
    ));
    Ok(resources.has_enough(&resources_for_pubdata))
}

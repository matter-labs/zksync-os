use constants::{CALLDATA_NON_ZERO_BYTE_GAS_COST, CALLDATA_ZERO_BYTE_GAS_COST};
use evm_interpreter::native_resource_constants::COPY_BYTE_NATIVE_COST;
use evm_interpreter::ERGS_PER_GAS;
use zk_ee::internal_error;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::{Computational, Resources};

use super::*;

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

pub fn get_resources_for_tx<S: EthereumLikeTypes>(
    gas_limit: u64,
    native_per_pubdata: U256,
    native_per_gas: U256,
    calldata: &[u8],
    intrinsic_gas: u64,
    intrinsic_pubdata: u64,
    intrinsic_native: u64,
) -> Result<ResourcesForTx<S>, TxError> {
    // TODO: operator trusted gas limit?

    // This is the real limit, which we later use to compute native_used.
    // From it, we discount intrinsic pubdata and then take the min
    // with the MAX_NATIVE_COMPUTATIONAL.
    // We do those operations in that order because the pubdata charge
    // isn't computational.
    // We can consider in the future to keep two limits, so that pubdata
    // is not charged from computational resource.
    // Note: if native_per_gas is 0, we treat it as unlimited_native.
    // This can only happen when gas_price is 0, which means that fees
    // aren't charged.
    let native_limit = if cfg!(feature = "unlimited_native") || native_per_gas.is_zero() {
        u64::MAX
    } else {
        gas_limit.saturating_mul(u256_to_u64_saturated(&native_per_gas))
    };

    // Charge pubdata overhead
    let intrinsic_pubdata_overhead = u256_to_u64_saturated(&native_per_pubdata)
        .checked_mul(intrinsic_pubdata)
        .ok_or(internal_error!("npp*ip"))?;
    let native_limit =
        native_limit
            .checked_sub(intrinsic_pubdata_overhead)
            .ok_or(TxError::Validation(
                errors::InvalidTransaction::OutOfNativeResourcesDuringValidation,
            ))?;

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
    let (calldata_gas, calldata_native) = cost_for_calldata(calldata)?;

    let intrinsic_computational_native_charged = calldata_native
        .checked_add(intrinsic_native)
        .ok_or(TxError::Validation(
            errors::InvalidTransaction::OutOfNativeResourcesDuringValidation,
        ))?;

    let native_limit = native_limit
        .checked_sub(intrinsic_computational_native_charged)
        .ok_or(TxError::Validation(
            errors::InvalidTransaction::OutOfNativeResourcesDuringValidation,
        ))?;

    let native_limit =
        <<S as zk_ee::system::SystemTypes>::Resources as Resources>::Native::from_computational(
            native_limit,
        );

    // Intrinsic overhead
    let intrinsic_overhead = intrinsic_gas;

    let total_gas_to_charge = calldata_gas
        .checked_add(intrinsic_overhead)
        .ok_or(internal_error!("tuo+io"))?;

    if total_gas_to_charge > gas_limit {
        Err(TxError::Validation(
            errors::InvalidTransaction::OutOfGasDuringValidation,
        ))
    } else {
        let gas_limit_for_tx = gas_limit - total_gas_to_charge;
        let ergs = gas_limit_for_tx
            .checked_mul(ERGS_PER_GAS)
            .ok_or(internal_error!("glft*EPF"))?;
        let main_resources = S::Resources::from_ergs_and_native(Ergs(ergs), native_limit);
        Ok(ResourcesForTx {
            main_resources,
            withheld,
            intrinsic_computational_native_charged,
        })
    }
}
///
/// Computes the (gas, native) cost for the transaction's calldata.
///
pub fn cost_for_calldata(calldata: &[u8]) -> Result<(u64, u64), InternalError> {
    let zero_bytes = calldata.iter().filter(|byte| **byte == 0).count() as u64;
    let non_zero_bytes = calldata.len() as u64 - zero_bytes;
    let zero_cost = zero_bytes
        .checked_mul(CALLDATA_ZERO_BYTE_GAS_COST)
        .ok_or(internal_error!("zb*CZBGC"))?;
    let non_zero_cost = non_zero_bytes
        .checked_mul(CALLDATA_NON_ZERO_BYTE_GAS_COST)
        .ok_or(internal_error!("nzb*CNZBGC"))?;
    let gas_cost = zero_cost
        .checked_add(non_zero_cost)
        .ok_or(internal_error!("zc+nzc"))?;
    let native_cost = (calldata.len() as u64)
        .checked_mul(COPY_BYTE_NATIVE_COST)
        .ok_or(internal_error!("cl*CBNC"))?;
    Ok((gas_cost, native_cost))
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
    native_per_pubdata: U256,
    base_pubdata: Option<u64>,
) -> Result<(u64, S::Resources), InternalError> {
    let current_pubdata_spent = system
        .net_pubdata_used()?
        .saturating_sub(base_pubdata.unwrap_or(0));
    let native_per_pubdata = u256_to_u64_saturated(&native_per_pubdata);
    let native = current_pubdata_spent
        .checked_mul(native_per_pubdata)
        .ok_or(internal_error!("cps*epp"))?;
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
    native_per_pubdata: U256,
    resources: &S::Resources,
    base_pubdata: Option<u64>,
) -> Result<(bool, S::Resources, u64), InternalError> {
    let (pubdata_used, resources_for_pubdata) =
        get_resources_to_charge_for_pubdata(system, native_per_pubdata, base_pubdata)?;
    let _ = system.get_logger().write_fmt(format_args!(
        "Checking gas for pubdata, resources_for_pubdata: {resources_for_pubdata:?}, resources: {resources:?}\n"
    ));
    let enough = resources.has_enough(&resources_for_pubdata);
    Ok((enough, resources_for_pubdata, pubdata_used))
}

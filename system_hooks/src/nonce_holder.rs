use super::*;
use crate::addresses_constants::NONCE_HOLDER_HOOK_ADDRESS;
use core::fmt::Write;
use ruint::aliases::B160;
use zk_ee::system::errors::SystemError;
use zk_ee::system::logger::Logger;
use zk_ee::system::reference_implementations::BaseComputationalResources;
use zk_ee::system::CallModifier;
use zk_ee::system::Resources;

// emulates a behavior of NonceHodler contract in ZKSync Era

// it's actually stateless since we can immediately delegate a functionality to system itself

pub fn nonce_holder_hook<S: EthereumLikeSystemExt>(
    request: ExternalCallRequest<S::UsermodeSystem>,
    _caller_ee: u8,
    system: &mut S,
) -> Result<CompletedExecution<S::UsermodeSystem>, InternalError>
where
    S::IO: EthereumLikeIOSubsystemExt,
    <<S as SystemExt>::IO as IOSubsystemExt>::IO: EthereumLikeIOSubsystem,
    S::UsermodeSystem: EthereumLikeSystem<IO = <<S as SystemExt>::IO as IOSubsystemExt>::IO>,
{
    let ExternalCallRequest {
        desired_resources_to_pass,
        call_values,
        call_parameters,
    } = request;

    let CallValues {
        calldata,
        call_scratch_space: _,
        nominal_token_value,
    } = call_values;
    let CallParameters {
        callers_caller: _,
        caller,
        callee,
        modifier,
    } = call_parameters;

    debug_assert_eq!(callee, NONCE_HOLDER_HOOK_ADDRESS);

    let mut error = false;
    error |= nominal_token_value != U256::ZERO;
    error |= calldata.len() == 0;
    let mut is_static = false;
    match modifier {
        CallModifier::Constructor => {
            return Err(InternalError(
                "Nonce holder hook called with constructor modifier",
            ))
        }
        CallModifier::Static | CallModifier::ZKVMSystemStatic | CallModifier::DelegateStatic => {
            is_static = true;
        }
        _ => {}
    }

    // TODO: ensure onlySystemCall

    if error {
        return Ok(make_error_return_state(desired_resources_to_pass));
    }

    let mut resources = desired_resources_to_pass;

    let result = nonce_holder_hook_inner(calldata, &mut resources, system, caller, is_static);

    match result {
        Ok(Ok(range)) => Ok(make_return_state_from_heap_data(system, resources, range)?),
        Ok(Err(e)) => {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Revert: {:?}\n", e));
            Ok(make_error_return_state(resources))
        }
        Err(SystemError::OutOfResources) => {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Out of gas during system hook\n"));
            Ok(make_error_return_state(resources))
        }
        Err(SystemError::Internal(e)) => Err(e),
    }
}
// e1239cd8
const INCREMENT_MIN_NONCE_IF_EQUALS_SELECTOR: &[u8] = &[0xe1, 0x23, 0x9c, 0xd8];

// For now, we use the system nonce for both deployment and transaction
pub fn nonce_holder_hook_inner<S: EthereumLikeSystemExt>(
    calldata: OSImmutableSlice<S::UsermodeSystem>,
    resources: &mut BaseResources,
    system: &mut S,
    caller: B160,
    is_static: bool,
) -> Result<Result<core::ops::Range<usize>, &'static str>, SystemError>
where
    S::IO: EthereumLikeIOSubsystemExt,
    <<S as SystemExt>::IO as IOSubsystemExt>::IO: EthereumLikeIOSubsystem,
    S::UsermodeSystem: EthereumLikeSystem<IO = <<S as SystemExt>::IO as IOSubsystemExt>::IO>,
{
    const STEP_COST: BaseResources = BaseResources {
        spendable: BaseComputationalResources { ergs: 10 },
    };
    resources
        .spendable_part_mut()
        .try_spend_or_floor_self(STEP_COST.spendable_part())?;
    let calldata_slice = system.get_memory_region_range(calldata);
    let mut selector = [0u8; 4];
    selector.copy_from_slice(&calldata_slice[..4]);
    let _ = system
        .get_logger()
        .write_fmt(format_args!("Calldata for nonce holder:"));
    let _ = system.get_logger().log_data(calldata_slice.iter().copied());

    match selector {
        s if s == INCREMENT_MIN_NONCE_IF_EQUALS_SELECTOR => {
            if is_static {
                return Ok(Err(
                    "Nonce holder failure: increment nonce called with static context",
                ));
            }
            // TODO: can panic with calldata_length > 36
            let tx_nonce = U256::from_be_slice(&calldata_slice[4..]);
            let (acc_nonce, _) = read_nonce(system, &caller, resources)?;
            if tx_nonce == U256::from(acc_nonce) {
                match bump_nonce(system, &caller, resources) {
                    Ok(_) => Ok(Ok(0..0)),
                    Err(_) => Ok(Err("Nonce holder failure: error while updating nonce")),
                }
            } else {
                Ok(Err("Nonce holder failure: mismatched nonce"))
            }
        }
        _ => Ok(Err("Nonce holder failure: unknown selector")),
    }
}

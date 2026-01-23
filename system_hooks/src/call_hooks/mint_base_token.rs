use super::super::*;
use core::fmt::Write;
use ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::errors::{runtime::RuntimeError, system::SystemError};
use zk_ee::{internal_error, system_log};

/// System hook that allows the L2 base token contract to mint tokens
///
/// ## Usage
/// - Only callable by L2_BASE_TOKEN_ADDRESS (0x800a)
/// - Expects exactly 32 bytes of calldata containing the mint amount as U256 big-endian
/// - Increases the caller's nominal token balance by the specified amount
/// - Fails if called in static context or with invalid calldata
pub fn mint_base_token_hook<'a, S: EthereumLikeTypes>(
    request: ExternalCallRequest<S>,
    _caller_ee: u8,
    system: &mut System<S>,
    return_memory: &'a mut [MaybeUninit<u8>],
) -> Result<(CompletedExecution<'a, S>, &'a mut [MaybeUninit<u8>]), SystemError>
where
    S::IO: IOSubsystemExt,
{
    let ExternalCallRequest {
        available_resources,
        ergs_to_pass: _,
        input: calldata,
        call_scratch_space: _,
        nominal_token_value: _,
        caller,
        callee,
        callers_caller: _,
        modifier,
    } = request;

    debug_assert_eq!(callee, MINT_HOOK_ADDRESS);

    // Only allow L2 base token contract to mint tokens
    if caller != L2_BASE_TOKEN_ADDRESS {
        // Pretend to be an empty account
        return Ok((
            make_return_state_from_returndata_region(available_resources, &[]),
            return_memory,
        ));
    }

    let mut error = false;
    let mut is_static = false;
    match modifier {
        CallModifier::Constructor => {
            return Err(internal_error!("Mint hook called with constructor modifier").into())
        }
        CallModifier::Delegate
        | CallModifier::DelegateStatic
        | CallModifier::EVMCallcode
        | CallModifier::EVMCallcodeStatic => {
            error = true;
        }
        CallModifier::Static | CallModifier::ZKVMSystemStatic => {
            is_static = true;
        }
        _ => {}
    }

    if error {
        return Ok((make_error_return_state(available_resources), return_memory));
    }

    let mut resources = available_resources;

    // Charge EVM gas for the mint operation. This is a temporary solution, so we don't care about EVM compatibility anyway
    evm_interpreter::charge_native_and_ergs::<S::Resources>(
        &mut resources,
        HOOK_BASE_NATIVE_COST,
        HOOK_BASE_ERGS_COST,
    )?;
    // Calldata length shouldn't be able to overflow u32, due to gas
    // limitations.
    let calldata_len: u32 = calldata
        .len()
        .try_into()
        .map_err(|_| internal_error!("Calldata is larger than u32"))?;

    let result = mint(
        calldata,
        calldata_len,
        &mut resources,
        system,
        caller,
        is_static,
    );

    // We should never revert in practice, so it's ok to break EVM compatibility there
    match result {
        Ok(Ok(_)) => {
            // Successful mint - return empty data
            Ok((
                make_return_state_from_returndata_region(resources, &[]),
                return_memory,
            ))
        }
        Ok(Err(e)) => {
            system_log!(system, "Revert: {e:?}\n");
            Ok((make_error_return_state(resources), return_memory))
        }
        Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
            system_log!(system, "Out of gas during system hook\n");
            Ok((make_error_return_state(resources), return_memory))
        }
        Err(e @ SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(_))) => Err(e),
        Err(SystemError::LeafDefect(e)) => Err(e.into()),
    }
}

/// Core minting logic that validates input and performs the token mint operation.
#[allow(clippy::too_many_arguments)]
fn mint<S: EthereumLikeTypes>(
    calldata: &[u8],
    calldata_len: u32,
    resources: &mut S::Resources,
    system: &mut System<S>,
    caller: B160,
    is_static: bool,
) -> Result<Result<(), &'static str>, SystemError>
where
    S::IO: IOSubsystemExt,
{
    if is_static {
        return Ok(Err("Mint hook failure: mint called with static context"));
    }

    if calldata_len != 32 {
        return Ok(Err("Mint hook failure: mint called with invalid calldata"));
    }

    let nominal_token_value = U256::from_be_slice(&calldata);

    mint_nominal_token_value(resources, system, &caller, &nominal_token_value)?;

    Ok(Ok(()))
}

/// Updates the account's nominal token balance by adding the specified mint amount.
fn mint_nominal_token_value<S: EthereumLikeTypes>(
    resources: &mut S::Resources,
    system: &mut System<S>,
    beneficiary: &B160,
    nominal_token_value: &U256,
) -> Result<(), SystemError>
where
    S::IO: IOSubsystemExt,
{
    // Charge EVM gas for the mint operation. This is a temporary solution, so we don't care about EVM compatibility anyway
    match system.io.update_account_nominal_token_balance(
        ExecutionEnvironmentType::EVM,
        resources,
        beneficiary,
        &nominal_token_value,
        false, // false = add to balance, true = subtract from balance
    ) {
        Ok(_) => Ok(()),
        Err(SubsystemError::LeafUsage(_)) => Err(SystemError::LeafDefect(internal_error!(
            "Mint should be successful"
        ))),
        Err(SubsystemError::LeafRuntime(e)) => Err(e.into()),
        Err(SubsystemError::LeafDefect(e)) => Err(e.into()),
        Err(SubsystemError::Cascaded(e)) => match e {},
    }
}

//!
//! Set bytecode on address system hook implementation.
//! This hook allows setting deployed EVM bytecode to any address.
//! It's used exclusively for protocol upgrades approved by governance.
//!
use super::super::*;
use crate::addresses_constants::{CONTRACT_DEPLOYER_ADDRESS, SET_BYTECODE_ON_ADDRESS_HOOK};
use core::fmt::Write;
use evm_interpreter::MAX_CODE_SIZE;
use ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::{runtime::RuntimeError, system::SystemError};
use zk_ee::utils::Bytes32;
use zk_ee::{internal_error, out_of_return_memory, system_log};

pub fn set_bytecode_on_address_hook<'a, S: EthereumLikeTypes>(
    request: ExternalCallRequest<S>,
    caller_ee: u8,
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
        nominal_token_value,
        caller,
        callee,
        callers_caller: _,
        modifier,
    } = request;

    debug_assert_eq!(callee, SET_BYTECODE_ON_ADDRESS_HOOK);

    // Can be used only by Contract Deployer system contract
    if caller != CONTRACT_DEPLOYER_ADDRESS {
        system_log!(
            system,
            "Set bytecode hook: invalid caller (caller={caller:?})\n"
        );
        // Pretend to be an empty account
        return Ok((
            make_return_state_from_returndata_region(available_resources, &[]),
            return_memory,
        ));
    }

    // Note: it's ok to revert below even when it breaks EVM compatibility, since
    // this hook should be used in non-EVM-compatible context only (protocol upgrade).

    // This hook doesn't accept any native token value
    let mut error = nominal_token_value != U256::ZERO;
    let mut is_static = false;
    match modifier {
        CallModifier::Constructor => {
            return Err(internal_error!(
                "Set bytecode on address hook called with constructor modifier"
            )
            .into())
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

    let result =
        set_bytecode_on_address_hook_inner(&calldata, &mut resources, system, caller_ee, is_static);

    match result {
        Ok(Ok(return_data)) => {
            let mut return_memory = SliceVec::new(return_memory);
            return_memory
                .try_extend(return_data.iter().copied())
                .map_err(|_| out_of_return_memory!())?;
            let (returndata, rest) = return_memory.destruct();
            Ok((
                make_return_state_from_returndata_region(resources, returndata),
                rest,
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

fn set_bytecode_on_address_hook_inner<S: EthereumLikeTypes>(
    calldata: &[u8],
    resources: &mut S::Resources,
    system: &mut System<S>,
    _caller_ee: u8,
    is_static: bool,
) -> Result<Result<&'static [u8], &'static str>, SystemError>
where
    S::IO: IOSubsystemExt,
{
    evm_interpreter::charge_native_and_ergs::<S::Resources>(
        resources,
        HOOK_BASE_NATIVE_COST,
        HOOK_BASE_ERGS_COST,
    )?;

    if is_static {
        return Ok(Err(
            "Set bytecode on address failure: called with static context",
        ));
    }

    if calldata.len() < 128 {
        return Ok(Err(
            "Set bytecode on address failure: called with invalid calldata",
        ));
    }

    // Check that the first 12 bytes are zero (ABI left-padding for address)
    if calldata[..12].iter().any(|&b| b != 0) {
        return Err(SystemError::LeafDefect(internal_error!(
            "Address word is not ABI-encoded (upper 12 bytes non-zero)"
        )));
    }

    let address = B160::try_from_be_slice(&calldata[12..32]).ok_or(SystemError::LeafDefect(
        internal_error!("Failed to create B160 from 20 byte array"),
    ))?;

    let bytecode_hash = Bytes32::from_array(calldata[32..64].try_into().expect("Always valid"));

    let bytecode_length: u32 = match U256::from_be_slice(&calldata[64..96]).try_into() {
        Ok(length) => length,
        Err(_) => {
            return Ok(Err(
                "Set bytecode on address failure: called with invalid calldata",
            ))
        }
    };

    let observable_bytecode_hash =
        Bytes32::from_array(calldata[96..128].try_into().expect("Always valid"));

    // Although this can be called as a part of protocol upgrade,
    // we are checking the next invariants, just in case
    // EIP-158: reject code of length > 24576.
    if bytecode_length as usize > MAX_CODE_SIZE {
        return Ok(Err(
            "Set bytecode on address failure: called with invalid bytecode(length > 24576)",
        ));
    }
    // Also EIP-3541(reject code starting with 0xEF) should be validated by governance.

    // Charge extra ergs for `set_bytecode_details`
    let ergs = set_bytecode_details_extra_ergs(bytecode_length);
    resources.charge(&S::Resources::from_ergs(ergs))?;

    system.set_bytecode_details(
        resources,
        &address,
        ExecutionEnvironmentType::EVM,
        bytecode_hash,
        bytecode_length,
        0,
        observable_bytecode_hash,
        bytecode_length, // observable_bytecode_length is equal to bytecode_length here
    )?;

    Ok(Ok(&[]))
}

///
/// We add some ergs cost to account for work charged in native only.
/// This is:
///  - Getting preimage of [bytecode_len] length.
///  - Creating artifacts for code.
///  - Hashing (Blake2s) bytecode+artifacts.
///
/// Note that the IO access gas cost is added by set_bytecode_details.
/// Instead of doing a fine-grained calculation, we pick a constant
/// (to be multiplied by the bytecode length) that should be big enough
/// to cover for this.
/// Note: the native resources still protect us from DoS in case this
/// approximation is too low.
///
fn set_bytecode_details_extra_ergs(bytecode_len: u32) -> Ergs {
    SET_BYTECODE_DETAILS_EXTRA_ERGS_PER_BYTE.times(bytecode_len as u64)
}

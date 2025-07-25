//!
//! Contract deployer system hook implementation.
//! It implements a `setDeployedCodeEVM` method, similar to Era.
//! It's needed for protocol upgrades.
//!
use super::*;
use core::fmt::Write;
use evm_interpreter::MAX_CODE_SIZE;
use ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::system::errors::{runtime::RuntimeError, system::SystemError};
use zk_ee::utils::Bytes32;

pub fn contract_deployer_hook<'a, S: EthereumLikeTypes>(
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
        calldata,
        call_scratch_space: _,
        nominal_token_value,
        caller,
        callee,
        callers_caller: _,
        modifier,
    } = request;

    debug_assert_eq!(callee, CONTRACT_DEPLOYER_ADDRESS);

    // There is no "payable" methods
    let mut error = nominal_token_value != U256::ZERO;
    let mut is_static = false;
    match modifier {
        CallModifier::Constructor => {
            return Err(
                internal_error!("Contract deployer hook called with constructor modifier").into(),
            )
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

    let result = contract_deployer_hook_inner(
        &calldata,
        &mut resources,
        system,
        caller,
        caller_ee,
        is_static,
    );

    Ok((
        match result {
            Ok(Ok(return_data)) => make_return_state_from_returndata_region(resources, return_data),
            Ok(Err(e)) => {
                let _ = system
                    .get_logger()
                    .write_fmt(format_args!("Revert: {:?}\n", e));
                make_error_return_state(resources)
            }
            Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
                let _ = system
                    .get_logger()
                    .write_fmt(format_args!("Out of gas during system hook\n"));
                make_error_return_state(resources)
            }
            Err(e @ SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(_))) => {
                return Err(e)
            }
            Err(SystemError::LeafDefect(e)) => return Err(e.into()),
        },
        return_memory,
    ))
}

// setBytecodeDetailsEVM(address,bytes32,uint32,bytes32) - f6eca0b0
const SET_EVM_BYTECODE_DETAILS: &[u8] = &[0xf6, 0xec, 0xa0, 0xb0];
const L2_COMPLEX_UPGRADER_ADDRESS: B160 = B160::from_limbs([0x800f, 0, 0]);

fn contract_deployer_hook_inner<S: EthereumLikeTypes>(
    mut calldata: &[u8],
    resources: &mut S::Resources,
    system: &mut System<S>,
    caller: B160,
    _caller_ee: u8,
    is_static: bool,
) -> Result<Result<&'static [u8], &'static str>, SystemError>
where
    S::IO: IOSubsystemExt,
{
    // TODO: charge native
    let step_cost: S::Resources = S::Resources::from_ergs(Ergs(10));
    resources.charge(&step_cost)?;

    if calldata.len() < 4 {
        return Ok(Err(
            "Contract deployer hook failure: calldata shorter than selector length",
        ));
    }
    let mut selector = [0u8; 4];
    selector.copy_from_slice(&calldata[..4]);

    match selector {
        s if s == SET_EVM_BYTECODE_DETAILS => {
            if is_static {
                return Ok(Err(
                    "Contract deployer failure: setBytecodeDetailsEVM called with static context",
                ));
            }
            // in future we need to handle regular(not genesis) protocol upgrades
            if caller != L2_COMPLEX_UPGRADER_ADDRESS {
                return Ok(Err(
                    "Contract deployer failure: unauthorized caller for setBytecodeDetailsEVM",
                ));
            }

            // decoding according to setBytecodeDetailsEVM(address,bytes32,uint32,bytes32)
            calldata = &calldata[4..];
            if calldata.len() < 128 {
                return Ok(Err(
                    "Contract deployer failure: setBytecodeDetailsEVM called with invalid calldata",
                ));
            }

            // check that first 12 bytes in address encoding are zero
            if calldata[0..12].iter().any(|byte| *byte != 0) {
                return Ok(Err(
                    "Contract deployer failure: setBytecodeDetailsEVM called with invalid calldata",
                ));
            }
            let address =
                B160::try_from_be_slice(&calldata[12..32]).ok_or(SystemError::LeafDefect(
                    internal_error!("Failed to create B160 from 20 byte array"),
                ))?;

            let bytecode_hash =
                Bytes32::from_array(calldata[32..64].try_into().expect("Always valid"));

            let bytecode_length: u32 = match U256::from_be_slice(&calldata[64..96]).try_into() {
                Ok(length) => length,
                Err(_) => return Ok(Err(
                    "Contract deployer failure: setBytecodeDetailsEVM called with invalid calldata",
                )),
            };

            let observable_bytecode_hash =
                Bytes32::from_array(calldata[96..128].try_into().expect("Always valid"));

            // Although this can be called as a part of protocol upgrade,
            // we are checking the next invariants, just in case
            // EIP-158: reject code of length > 24576.
            if bytecode_length as usize > MAX_CODE_SIZE {
                return Ok(Err(
                    "Contract deployer failure: setBytecodeDetailsEVM called with invalid bytecode(length > 24576)",
                ));
            }
            // Also EIP-3541(reject code starting with 0xEF) should be validated by governance.

            system.set_bytecode_details(
                resources,
                &address,
                ExecutionEnvironmentType::EVM,
                bytecode_hash,
                bytecode_length,
                0,
                observable_bytecode_hash,
                bytecode_length,
            )?;

            Ok(Ok(&[]))
        }
        _ => Ok(Err("Contract deployer hook: unknown selector")),
    }
}

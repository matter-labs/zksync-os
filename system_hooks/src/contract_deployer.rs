//!
//! Contract deployer system hook implementation.
//! It implements a `setDeployedCodeEVM` method, similar to Era.
//! It's needed for protocol upgrades.
//!
use super::*;
use core::fmt::Write;
use evm_interpreter::MAX_CODE_SIZE;
use ruint::aliases::B160;
use u256::U256;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::SystemError;

pub fn contract_deployer_hook<'a, S: EthereumLikeTypes>(
    request: ExternalCallRequest<S>,
    caller_ee: u8,
    system: &mut System<S>,
    return_memory: &'a mut [MaybeUninit<u8>],
) -> Result<(CompletedExecution<'a, S>, &'a mut [MaybeUninit<u8>]), FatalError>
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
    let mut error = nominal_token_value != U256::zero();
    let mut is_static = false;
    match modifier {
        CallModifier::Constructor => {
            return Err(
                InternalError("Contract deployer hook called with constructor modifier").into(),
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
            Err(SystemError::OutOfErgs) => {
                let _ = system
                    .get_logger()
                    .write_fmt(format_args!("Out of gas during system hook\n"));
                make_error_return_state(resources)
            }
            Err(SystemError::OutOfNativeResources) => return Err(FatalError::OutOfNativeResources),
            Err(SystemError::Internal(e)) => return Err(e.into()),
        },
        return_memory,
    ))
}

// setDeployedCodeEVM(address,bytes) - 1223adc7
const SET_DEPLOYED_CODE_EVM_SELECTOR: &[u8] = &[0x12, 0x23, 0xad, 0xc7];
const L2_GENESIS_UPGRADE_ADDRESS: B160 = B160::from_limbs([0x10001, 0, 0]);

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
        s if s == SET_DEPLOYED_CODE_EVM_SELECTOR => {
            if is_static {
                return Ok(Err(
                    "Contract deployer failure: setDeployedCodeEVM called with static context",
                ));
            }
            // in future we need to handle regular(not genesis) protocol upgrades
            if caller != L2_GENESIS_UPGRADE_ADDRESS {
                return Ok(Err(
                    "Contract deployer failure: unauthorized caller for setDeployedCodeEVM",
                ));
            }

            // decoding according to setDeployedCodeEVM(address,bytes)
            calldata = &calldata[4..];
            if calldata.len() < 64 {
                return Ok(Err(
                    "Contract deployer failure: setDeployedCodeEVM called with invalid calldata",
                ));
            }

            // check that first 12 bytes in address encoding are zero
            if calldata[0..12].iter().any(|byte| *byte != 0) {
                return Ok(Err(
                    "Contract deployer failure: setDeployedCodeEVM called with invalid calldata",
                ));
            }
            let address = B160::try_from_be_slice(&calldata[12..32]).ok_or(
                SystemError::Internal(InternalError("Failed to create B160 from 20 byte array")),
            )?;

            let bytecode_offset: usize = match U256::try_from_be_slice(&calldata[32..64])
                .expect("Should convert slice to U256")
                .try_into()
            {
                Ok(offset) => offset,
                Err(_) => return Ok(Err(
                    "Contract deployer failure: setDeployedCodeEVM called with invalid calldata",
                )),
            };

            let bytecode_length_encoding_end = match bytecode_offset.checked_add(32) {
                Some(deployments_encoding_end) => deployments_encoding_end,
                None => return Ok(Err(
                    "Contract deployer failure: setDeployedCodeEVM called with invalid calldata",
                )),
            };
            let bytecode_length: usize = match U256::try_from_be_slice(
                &calldata[bytecode_length_encoding_end - 32..bytecode_length_encoding_end],
            )
            .expect("Should convert slice to U256")
            .try_into()
            {
                Ok(length) => length,
                Err(_) => return Ok(Err(
                    "Contract deployer failure: setDeployedCodeEVM called with invalid calldata",
                )),
            };

            if calldata.len() < bytecode_length_encoding_end + bytecode_length {
                return Ok(Err(
                    "Contract deployer failure: setDeployedCodeEVM called with invalid calldata",
                ));
            }

            let bytecode = &calldata
                [bytecode_length_encoding_end..bytecode_length_encoding_end + bytecode_length];

            // Although this can be called as a part of protocol upgrade,
            // we are checking the next invariants, just in case
            // EIP-3541: reject code starting with 0xEF.
            // EIP-158: reject code of length > 24576.
            if !bytecode.is_empty() && bytecode[0] == 0xEF || bytecode.len() > MAX_CODE_SIZE {
                return Ok(Err(
                    "Contract deployer failure: setDeployedCodeEVM called with invalid bytecode(it starts with 0xEF or length > 24576)",
                ));
            }

            system.io.deploy_code(
                ExecutionEnvironmentType::EVM,
                resources,
                &address,
                bytecode,
                bytecode.len() as u32,
                0,
            )?;

            Ok(Ok(&[]))
        }
        _ => Ok(Err("Contract deployer hook: unknown selector")),
    }
}

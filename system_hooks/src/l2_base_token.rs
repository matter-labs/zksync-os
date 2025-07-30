//!
//! L2 base token system hook implementation.
//! It implements methods for `withdraw` and `withdrawWithMessage`,
//! which work in the same way as in Era.
//!
use super::*;
use core::fmt::Write;
use ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::errors::{runtime::RuntimeError, system::SystemError};
use zk_ee::system::logger::Logger;

pub fn l2_base_token_hook<'a, S: EthereumLikeTypes>(
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

    debug_assert_eq!(callee, L2_BASE_TOKEN_ADDRESS);

    let mut error = false;
    let mut is_static = false;
    match modifier {
        CallModifier::Constructor => {
            return Err(
                internal_error!("L2 base token hook called with constructor modifier").into(),
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

    let result = l2_base_token_hook_inner(
        &calldata,
        &mut resources,
        system,
        caller,
        caller_ee,
        nominal_token_value,
        is_static,
    );

    match result {
        Ok(Ok(return_data)) => Ok(make_return_state_from_returndata_region(
            resources,
            return_data,
        )),
        Ok(Err(e)) => {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Revert: {e:?}\n"));
            Ok(make_error_return_state(resources))
        }
        Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Out of gas during system hook\n"));
            Ok(make_error_return_state(resources))
        }
        Err(e @ SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(_))) => Err(e),
        Err(SystemError::LeafDefect(e)) => Err(e.into()),
    }
    .map(|x| (x, return_memory))
}

// withdraw(address) - 51cff8d9
const WITHDRAW_SELECTOR: &[u8] = &[0x51, 0xcf, 0xf8, 0xd9];

// withdrawWithMessage(address,bytes) - 84bc3eb0
const WITHDRAW_WITH_MESSAGE_SELECTOR: &[u8] = &[0x84, 0xbc, 0x3e, 0xb0];

// finalizeEthWithdrawal(uint256,uint256,uint16,bytes,bytes32[]) - 6c0960f9
const FINALIZE_ETH_WITHDRAWAL_SELECTOR: &[u8] = &[0x6c, 0x09, 0x60, 0xf9];

fn l2_base_token_hook_inner<S: EthereumLikeTypes>(
    calldata: &[u8],
    resources: &mut S::Resources,
    system: &mut System<S>,
    caller: B160,
    caller_ee: u8,
    nominal_token_value: U256,
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
            "L2 base token failure: calldata shorter than selector length",
        ));
    }
    let mut selector = [0u8; 4];
    selector.copy_from_slice(&calldata[..4]);
    let _ = system
        .get_logger()
        .write_fmt(format_args!("Selector for l2 base token:"));
    let _ = system.get_logger().log_data(selector.iter().copied());

    match selector {
        s if s == WITHDRAW_SELECTOR => {
            if is_static {
                return Ok(Err(
                    "L2 base token failure: withdraw called with static context",
                ));
            }
            // following solidity abi for withdraw(address)
            if calldata.len() < 36 {
                return Ok(Err(
                    "L2 base token failure: withdraw called with invalid calldata",
                ));
            }
            // Burn nominal_token_value
            match system.io.update_account_nominal_token_balance(
                ExecutionEnvironmentType::parse_ee_version_byte(caller_ee)
                    .map_err(SystemError::LeafDefect)?,
                resources,
                &L2_BASE_TOKEN_ADDRESS,
                &nominal_token_value,
                true,
            ) {
                Ok(_) => Ok(()),
                // TODO this has to be properly propagated
                Err(SubsystemError::LeafUsage(_)) => Err(SystemError::LeafDefect(internal_error!(
                    "L2 base token must have withdrawal amount"
                ))),
                Err(SubsystemError::LeafRuntime(e)) => Err(e.into()),
                Err(SubsystemError::LeafDefect(e)) => Err(e.into()),
                Err(SubsystemError::Cascaded(e)) => match e {},
            }?;

            // Emit log
            // Packed ABI encoding of:
            // - IMailbox.finalizeEthWithdrawal.selector (4)
            // - l1_receiver (20)
            // - nominal_token_value (32)
            let mut message = [0u8; 56];
            message[0..4].copy_from_slice(FINALIZE_ETH_WITHDRAWAL_SELECTOR);
            // check that first 12 bytes in address encoding are zero
            if calldata[4..4 + 12].iter().any(|byte| *byte != 0) {
                return Ok(Err(
                    "Contract deployer failure: withdraw called with invalid calldata",
                ));
            }
            message[4..24].copy_from_slice(&calldata[(4 + 12)..36]);
            message[24..56].copy_from_slice(&nominal_token_value.to_be_bytes::<32>());
            system.io.emit_l1_message(
                ExecutionEnvironmentType::parse_ee_version_byte(caller_ee)
                    .map_err(SystemError::LeafDefect)?,
                resources,
                &L2_BASE_TOKEN_ADDRESS,
                &message,
            )?;

            // TODO: emit event for withdrawal for Era compatibility
            Ok(Ok(&[]))
        }
        s if s == WITHDRAW_WITH_MESSAGE_SELECTOR => {
            if is_static {
                return Ok(Err(
                    "L2 base token failure: withdrawWithMessage called with static context",
                ));
            }
            // following solidity abi for withdrawWithMessage(address,bytes)
            if calldata.len() < 68 {
                return Ok(Err(
                    "L2 base token failure: withdrawWithMessage called with invalid calldata",
                ));
            }
            let message_offset: usize =
                match U256::from_be_slice(&calldata[36..68]).try_into() {
                    Ok(offset) => offset,
                    Err(_) => return Ok(Err(
                        "L2 base token failure: withdrawWithMessage called with invalid calldata",
                    )),
                };
            // length located at 4+message_offset..4+message_offset+32
            // we want to check that 4+message_offset+32 will not overflow usize
            let length_encoding_end =
                match message_offset.checked_add(36) {
                    Some(length_encoding_end) => length_encoding_end,
                    None => return Ok(Err(
                        "L2 base token failure: withdrawWithMessage called with invalid calldata",
                    )),
                };
            if calldata.len() < length_encoding_end {
                return Ok(Err(
                    "L2 base token failure: withdrawWithMessage called with invalid calldata",
                ));
            }
            let length =
                match U256::from_be_slice(&calldata[length_encoding_end - 32..length_encoding_end])
                    .try_into()
                {
                    Ok(length) => length,
                    Err(_) => return Ok(Err(
                        "L2 base token failure: withdrawWithMessage called with invalid calldata",
                    )),
                };
            // to check that it will not overflow
            let message_end =
                match length_encoding_end.checked_add(length) {
                    Some(message_end) => message_end,
                    None => return Ok(Err(
                        "L2 base token failure: withdrawWithMessage called with invalid calldata",
                    )),
                };
            if calldata.len() < message_end {
                return Ok(Err(
                    "L2 base token failure: withdrawWithMessage called with invalid calldata",
                ));
            }
            let additional_data = &calldata[length_encoding_end..message_end];

            // Burn nominal_token_value
            match system.io.update_account_nominal_token_balance(
                ExecutionEnvironmentType::parse_ee_version_byte(caller_ee)
                    .map_err(SystemError::LeafDefect)?,
                resources,
                &L2_BASE_TOKEN_ADDRESS,
                &nominal_token_value,
                true,
            ) {
                Ok(_) => Ok(()),
                // TODO this has to be properly propagated
                Err(SubsystemError::LeafUsage(_)) => Err(SystemError::LeafDefect(internal_error!(
                    "L2 base token must have withdrawal amount"
                ))),
                Err(SubsystemError::LeafRuntime(e)) => Err(e.into()),
                Err(SubsystemError::LeafDefect(e)) => Err(e.into()),
                Err(SubsystemError::Cascaded(e)) => match e {},
            }?;

            // Emit log
            // Packed ABI encoding of:
            // - IMailbox.finalizeEthWithdrawal.selector (4)
            // - l1_receiver (20)
            // - nominal_token_value (32)
            // - sender (20)
            // - additional_data (length)
            let message_length = 76 + length;
            let mut message: alloc::vec::Vec<u8, S::Allocator> =
                alloc::vec::Vec::with_capacity_in(message_length, system.get_allocator());
            message.extend_from_slice(FINALIZE_ETH_WITHDRAWAL_SELECTOR);
            message.extend_from_slice(&calldata[16..36]);
            message.extend_from_slice(&nominal_token_value.to_be_bytes::<32>());
            message.extend_from_slice(&caller.to_be_bytes::<20>());
            message.extend_from_slice(additional_data);

            system.io.emit_l1_message(
                ExecutionEnvironmentType::parse_ee_version_byte(caller_ee)
                    .map_err(SystemError::LeafDefect)?,
                resources,
                &L2_BASE_TOKEN_ADDRESS,
                &message,
            )?;

            // TODO: emit event for Era compatibility
            Ok(Ok(&[]))
        }
        _ => Ok(Err("L2 base token: unknown selector")),
    }
}

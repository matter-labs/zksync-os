//!
//! L1 messenger system hook implementation.
//! It implements a `sendToL1` method, works same way as in Era.
//!
use super::*;
use ::u256::U256;
use core::fmt::Write;
use errors::FatalError;
use ruint::aliases::B160;
use zk_ee::{
    execution_environment_type::ExecutionEnvironmentType,
    system::{
        errors::SystemError, logger::Logger, CallModifier, CompletedExecution, ExternalCallRequest,
    },
    utils::Bytes32,
};

pub fn l1_messenger_hook<'a, S: EthereumLikeTypes>(
    request: ExternalCallRequest<S>,
    caller_ee: u8,
    system: &mut System<S>,
    return_memory: &'a mut [MaybeUninit<u8>],
) -> Result<(CompletedExecution<'a, S>, &'a mut [MaybeUninit<u8>]), FatalError>
where
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

    debug_assert_eq!(callee, L1_MESSENGER_ADDRESS);

    let mut error = false;
    // There is no "payable" methods
    error |= nominal_token_value.is_zero() == false;
    let mut is_static = false;
    match modifier {
        CallModifier::Constructor => {
            return Err(InternalError("L1 messenger hook called with constructor modifier").into())
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

    let result = l1_messenger_hook_inner(
        &calldata,
        &mut resources,
        system,
        caller,
        caller_ee,
        is_static,
    );

    match result {
        Ok(Ok(return_data)) => {
            let mut return_memory = SliceVec::new(return_memory);
            // TODO: check endianness
            return_memory.extend(return_data.as_u8_ref().iter().copied());
            let (returndata, rest) = return_memory.destruct();
            Ok((
                make_return_state_from_returndata_region(resources, returndata),
                rest,
            ))
        }
        Ok(Err(e)) => {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Revert: {:?}\n", e));
            Ok((make_error_return_state(resources), return_memory))
        }
        Err(SystemError::OutOfErgs) => {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Out of gas during system hook\n"));
            Ok((make_error_return_state(resources), return_memory))
        }
        Err(SystemError::OutOfNativeResources) => Err(FatalError::OutOfNativeResources),
        Err(SystemError::Internal(e)) => Err(e.into()),
    }
}
// sendToL1(bytes) - 62f84b24
const SEND_TO_L1_SELECTOR: &[u8] = &[0x62, 0xf8, 0x4b, 0x24];

fn l1_messenger_hook_inner<S: EthereumLikeTypes>(
    calldata: &[u8],
    resources: &mut S::Resources,
    system: &mut System<S>,
    caller: B160,
    caller_ee: u8,
    is_static: bool,
) -> Result<Result<Bytes32, &'static str>, SystemError>
where
{
    // TODO: charge native
    let step_cost: S::Resources = S::Resources::from_ergs(Ergs(10));
    resources.charge(&step_cost)?;

    if calldata.len() < 4 {
        return Ok(Err(
            "L1 messenger failure: calldata shorter than selector length",
        ));
    }
    let mut selector = [0u8; 4];
    selector.copy_from_slice(&calldata[..4]);
    let _ = system
        .get_logger()
        .write_fmt(format_args!("Selector for l1 messenger:"));
    let _ = system.get_logger().log_data(selector.iter().copied());

    match selector {
        s if s == SEND_TO_L1_SELECTOR => {
            if is_static {
                return Ok(Err(
                    "L1 messenger failure: sendToL1 called with static context",
                ));
            }
            // following solidity abi for sendToL1(bytes _message)
            if calldata.len() < 36 {
                return Ok(Err(
                    "L1 messenger failure: sendToL1 called with invalid calldata",
                ));
            }
            let message_offset: usize =
                match U256::from_be_bytes(&calldata[4..36].try_into().unwrap()).try_into() {
                    Ok(offset) => offset,
                    Err(_) => {
                        return Ok(Err(
                            "L1 messenger failure: sendToL1 called with invalid calldata",
                        ))
                    }
                };
            // length located at 4+message_offset..4+message_offset+32
            // we want to check that 4+message_offset+32 will not overflow usize
            let length_encoding_end = match message_offset.checked_add(36) {
                Some(length_encoding_end) => length_encoding_end,
                None => {
                    return Ok(Err(
                        "L1 messenger failure: sendToL1 called with invalid calldata",
                    ))
                }
            };
            if calldata.len() < length_encoding_end {
                return Ok(Err(
                    "L1 messenger failure: sendToL1 called with invalid calldata",
                ));
            }
            let length = match U256::from_be_bytes(
                &calldata[length_encoding_end - 32..length_encoding_end]
                    .try_into()
                    .unwrap(),
            )
            .try_into()
            {
                Ok(length) => length,
                Err(_) => {
                    return Ok(Err(
                        "L1 messenger failure: sendToL1 called with invalid calldata",
                    ))
                }
            };
            // to check that it will not overflow
            let message_end = match length_encoding_end.checked_add(length) {
                Some(message_end) => message_end,
                None => {
                    return Ok(Err(
                        "L1 messenger failure: sendToL1 called with invalid calldata",
                    ))
                }
            };
            if calldata.len() < message_end {
                return Ok(Err(
                    "L1 messenger failure: sendToL1 called with invalid calldata",
                ));
            }
            let message = &calldata[length_encoding_end..message_end];
            let message_hash = system.io.emit_l1_message(
                ExecutionEnvironmentType::parse_ee_version_byte(caller_ee)
                    .map_err(SystemError::Internal)?,
                resources,
                &caller,
                message,
            )?;
            Ok(Ok(message_hash))
        }
        _ => Ok(Err("L1 messenger: unknown selector")),
    }
}

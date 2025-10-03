//!
//! L2 base token system hook implementation.
//!
//! This module provides the withdrawal functionality for the L2 base token (ETH equivalent).
//! It implements methods for `withdraw` and `withdrawWithMessage`, which work in the same way as in Era.
//!
//! ## Supported Operations
//! - `withdraw(address)` - Burns L2 tokens and initiates withdrawal to L1 receiver
//! - `withdrawWithMessage(address,bytes)` - Burns L2 tokens with additional data for L1 processing
//!
//! ## Notes
//! - Minting is performed in the bootloader automatically with corresponding "Mint" events if L1->L2 or upgrade tx has some value attached
use crate::l1_messenger::send_to_l1_inner;

use super::*;
use arrayvec::ArrayVec;
use core::fmt::Write;
use ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::kv_markers::MAX_EVENT_TOPICS;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::errors::{runtime::RuntimeError, system::SystemError};
use zk_ee::system::logger::Logger;
use zk_ee::utils::{b160_to_u256, Bytes32};

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
        input: calldata,
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
        Err(e @ SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(_))) => Err(e),
        Err(SystemError::LeafDefect(e)) => Err(e.into()),
    }
    .map(|x| (x, return_memory))
}

// withdraw(address) - 51cff8d9
pub const WITHDRAW_SELECTOR: &[u8] = &[0x51, 0xcf, 0xf8, 0xd9];

// withdrawWithMessage(address,bytes) - 84bc3eb0
pub const WITHDRAW_WITH_MESSAGE_SELECTOR: &[u8] = &[0x84, 0xbc, 0x3e, 0xb0];

// finalizeEthWithdrawal(uint256,uint256,uint16,bytes,bytes32[]) - 6c0960f9
pub const FINALIZE_ETH_WITHDRAWAL_SELECTOR: &[u8] = &[0x6c, 0x09, 0x60, 0xf9];

// keccak256("Withdrawal(address,address,uint256)")
const WITHDRAWAL_TOPIC: [u8; 32] = [
    0x27, 0x17, 0xea, 0xd6, 0xb9, 0x20, 0x0d, 0xd2, 0x35, 0xaa, 0xd4, 0x68, 0xc9, 0x80, 0x9e, 0xa4,
    0x00, 0xfe, 0x33, 0xac, 0x69, 0xb5, 0xbf, 0xaa, 0x6d, 0x3e, 0x90, 0xfc, 0x92, 0x2b, 0x63, 0x98,
];

// keccak256("WithdrawalWithMessage(address,address,uint256,bytes)")
const WITHDRAWAL_WITH_MESSAGE_TOPIC: [u8; 32] = [
    0xc4, 0x05, 0xfe, 0x89, 0x58, 0x41, 0x0b, 0xba, 0xf0, 0xc7, 0x3b, 0x7a, 0x0c, 0x3e, 0x20, 0x85,
    0x9e, 0x86, 0xca, 0x16, 0x8a, 0x4c, 0x9b, 0x0d, 0xef, 0x9c, 0x54, 0xd2, 0x55, 0x5a, 0x30, 0x6b,
];

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
    // Calldata length shouldn't be able to overflow u32, due to gas
    // limitations.
    let calldata_len: u32 = calldata
        .len()
        .try_into()
        .map_err(|_| internal_error!("Calldata is larger than u32"))?;

    match selector {
        s if s == WITHDRAW_SELECTOR => withdraw(
            calldata,
            calldata_len,
            resources,
            system,
            caller,
            caller_ee,
            nominal_token_value,
            is_static,
        ),
        s if s == WITHDRAW_WITH_MESSAGE_SELECTOR => withdraw_with_message(
            calldata,
            calldata_len,
            resources,
            system,
            caller,
            caller_ee,
            nominal_token_value,
            is_static,
        ),
        _ => Ok(Err("L2 base token: unknown selector")),
    }
}

/// Handles withdraw(address) calls - burns tokens and sends L1 message
/// Emits Withdrawal event on success
#[allow(clippy::too_many_arguments)]
fn withdraw<S: EthereumLikeTypes>(
    calldata: &[u8],
    calldata_len: u32,
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
    if is_static {
        return Ok(Err(
            "L2 base token failure: withdraw called with static context",
        ));
    }
    // following solidity abi for withdraw(address)
    if calldata_len < 36 {
        return Ok(Err(
            "L2 base token failure: withdraw called with invalid calldata",
        ));
    }

    burn_nominal_token_value(resources, system, caller_ee, &nominal_token_value)?;

    // Sending L2->L1 message.
    // ABI-encoded messages should consist of the following:
    // 32 bytes offset (must be 32)
    // 32 bytes length of the message
    // followed by the message itself, padded to be a multiple of 32 bytes.
    // In this case, it is known that the message is 56 bytes long:
    // - IMailbox.finalizeEthWithdrawal.selector (4)
    // - l1_receiver (20)
    // - nominal_token_value (32)

    // So the padded message will be 64 bytes long.
    // Total length of the encoded message will be 32 + 32 + 64 = 128 bytes.
    let mut l1_messenger_calldata = [0u8; 128];
    l1_messenger_calldata[31] = 32; // offset
    l1_messenger_calldata[63] = 56; // length
    l1_messenger_calldata[64..68].copy_from_slice(FINALIZE_ETH_WITHDRAWAL_SELECTOR);
    // check that first 12 bytes in address encoding are zero
    // TODO: should we fail or silently truncate? Solidity would truncate iirc.
    if calldata[4..4 + 12].iter().any(|byte| *byte != 0) {
        return Ok(Err(
            "L2 base token failure: withdraw called with invalid calldata",
        ));
    }

    let l1_receiver = &calldata[(4 + 12)..36];

    l1_messenger_calldata[68..88].copy_from_slice(&l1_receiver);
    l1_messenger_calldata[88..120].copy_from_slice(&nominal_token_value.to_be_bytes::<32>());

    let result = send_to_l1_inner(
        &l1_messenger_calldata,
        resources,
        system,
        L2_BASE_TOKEN_ADDRESS,
        caller_ee,
    )?;

    // event Withdrawal(address indexed _l2Sender, address indexed _l1Receiver, uint256 _amount);

    let mut topics = ArrayVec::<Bytes32, MAX_EVENT_TOPICS>::new();
    topics.push(Bytes32::from_array(WITHDRAWAL_TOPIC)); // event signature
    topics.push(Bytes32::from_u256_be(&b160_to_u256(caller))); // _l2Sender
    topics.push(Bytes32::from_u256_be(&U256::from_be_slice(&l1_receiver))); // _l1Receiver

    system.io.emit_event(
        ExecutionEnvironmentType::parse_ee_version_byte(caller_ee)
            .map_err(SystemError::LeafDefect)?,
        resources,
        &L2_BASE_TOKEN_ADDRESS,
        &topics,
        &nominal_token_value.to_be_bytes::<32>(), // _amount
    )?;

    Ok(result.map(|_| &[] as &[u8]))
}

/// Handles withdrawWithMessage(address,bytes) calls - burns tokens and sends L1 message with additional data
/// Emits WithdrawalWithMessage event on success
#[allow(clippy::too_many_arguments)]
fn withdraw_with_message<S: EthereumLikeTypes>(
    calldata: &[u8],
    calldata_len: u32,
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
    if is_static {
        return Ok(Err(
            "L2 base token failure: withdrawWithMessage called with static context",
        ));
    }
    // following solidity abi for withdrawWithMessage(address,bytes)
    if calldata_len < 68 {
        return Ok(Err(
            "L2 base token failure: withdrawWithMessage called with invalid calldata",
        ));
    }
    let message_offset: u32 = match U256::from_be_slice(&calldata[36..68]).try_into() {
        Ok(offset) => offset,
        Err(_) => {
            return Ok(Err(
                "L2 base token failure: withdrawWithMessage called with invalid calldata",
            ))
        }
    };
    // length located at 4+message_offset..4+message_offset+32
    // we want to check that 4+message_offset+32 will not overflow u32
    let length_encoding_end = match message_offset.checked_add(36) {
        Some(length_encoding_end) => length_encoding_end,
        None => {
            return Ok(Err(
                "L2 base token failure: withdrawWithMessage called with invalid calldata",
            ))
        }
    };
    if calldata_len < length_encoding_end {
        return Ok(Err(
            "L2 base token failure: withdrawWithMessage called with invalid calldata",
        ));
    }
    let length: u32 = match U256::from_be_slice(
        &calldata[(length_encoding_end as usize) - 32..length_encoding_end as usize],
    )
    .try_into()
    {
        Ok(length) => length,
        Err(_) => {
            return Ok(Err(
                "L2 base token failure: withdrawWithMessage called with invalid calldata",
            ))
        }
    };
    // to check that it will not overflow
    let message_end = match length_encoding_end.checked_add(length) {
        Some(message_end) => message_end,
        None => {
            return Ok(Err(
                "L2 base token failure: withdrawWithMessage called with invalid calldata",
            ))
        }
    };
    if calldata_len < message_end {
        return Ok(Err(
            "L2 base token failure: withdrawWithMessage called with invalid calldata",
        ));
    }
    let additional_data = &calldata[(length_encoding_end as usize)..message_end as usize];

    // check that first 12 bytes in address encoding are zero
    // TODO: should we fail or silently truncate? Solidity would truncate iirc.
    if calldata[4..4 + 12].iter().any(|byte| *byte != 0) {
        return Ok(Err(
            "L2 base token failure: withdrawWithMessage called with invalid calldata",
        ));
    }

    burn_nominal_token_value(resources, system, caller_ee, &nominal_token_value)?;

    // Sending L2->L1 message.
    // ABI-encoded messages should consist of the following:
    // 32 bytes offset (must be 32)
    // 32 bytes length of the message
    // followed by the message itself, padded to be a multiple of 32 bytes.
    // In this case, the message will consist of the following:
    // Packed ABI encoding of:
    // - IMailbox.finalizeEthWithdrawal.selector (4)
    // - l1_receiver (20)
    // - nominal_token_value (32)
    // - sender (20)
    // - additional_data (length of additional_data)
    let message_length = 76 + length;
    let abi_encoded_message_length = 32 + 32 + message_length;
    let abi_encoded_message_length = if abi_encoded_message_length % 32 != 0 {
        abi_encoded_message_length + (32 - (abi_encoded_message_length % 32))
    } else {
        abi_encoded_message_length
    };
    let mut message: alloc::vec::Vec<u8, S::Allocator> = alloc::vec::Vec::with_capacity_in(
        abi_encoded_message_length as usize + 32,
        system.get_allocator(),
    );
    // Offset and length
    message.extend_from_slice(&[0u8; 64]);
    message[31] = 32; // offset
    message[32..64].copy_from_slice(&U256::from(message_length).to_be_bytes::<32>());
    message.extend_from_slice(FINALIZE_ETH_WITHDRAWAL_SELECTOR);
    let l1_receiver = &calldata[16..36];
    message.extend_from_slice(&l1_receiver);
    message.extend_from_slice(&nominal_token_value.to_be_bytes::<32>());
    message.extend_from_slice(&caller.to_be_bytes::<20>());
    message.extend_from_slice(additional_data);
    // Populating the rest of the message with zeros to make it a multiple of 32 bytes
    message.extend(core::iter::repeat_n(
        0u8,
        abi_encoded_message_length as usize - message.len(),
    ));

    let result = send_to_l1_inner(
        &message,
        resources,
        system,
        L2_BASE_TOKEN_ADDRESS,
        caller_ee,
    )?;

    /*
        event WithdrawalWithMessage(
            address indexed _l2Sender,
            address indexed _l1Receiver,
            uint256 _amount,
            bytes _additionalData
        );
    */

    let mut topics = ArrayVec::<Bytes32, MAX_EVENT_TOPICS>::new();
    topics.push(Bytes32::from_array(WITHDRAWAL_WITH_MESSAGE_TOPIC)); // event signature
    topics.push(Bytes32::from_u256_be(&b160_to_u256(caller))); // _l2Sender
    topics.push(Bytes32::from_u256_be(&U256::from_be_slice(&l1_receiver))); // _l1Receiver

    // ABI encode event data: _amount (32 bytes) + _additionalData offset (32) + length (32) + data
    let abi_encoded_event_length = 32 + 32 + 32 + additional_data.len();
    let abi_encoded_event_length = if abi_encoded_event_length % 32 != 0 {
        abi_encoded_event_length + (32 - (abi_encoded_event_length % 32))
    } else {
        abi_encoded_event_length
    };
    let mut event_data =
        alloc::vec::Vec::with_capacity_in(abi_encoded_event_length + 32, system.get_allocator());
    event_data.extend_from_slice(&nominal_token_value.to_be_bytes::<32>());
    event_data.extend_from_slice(&[0u8; 64]);
    event_data[63] = 64; // offset
    event_data[64..96].copy_from_slice(&U256::from(additional_data.len()).to_be_bytes::<32>());
    event_data.extend_from_slice(additional_data);
    // Populating the rest of the event data with zeros to make it a multiple of 32 bytes
    event_data.extend(core::iter::repeat_n(
        0u8,
        abi_encoded_event_length - additional_data.len(),
    ));

    system.io.emit_event(
        ExecutionEnvironmentType::parse_ee_version_byte(caller_ee)
            .map_err(SystemError::LeafDefect)?,
        resources,
        &L2_BASE_TOKEN_ADDRESS,
        &topics,
        &event_data,
    )?;

    Ok(result.map(|_| &[] as &[u8]))
}

/// Burns the specified amount of nominal tokens from the L2 base token contract
fn burn_nominal_token_value<S: EthereumLikeTypes>(
    resources: &mut S::Resources,
    system: &mut System<S>,
    caller_ee: u8,
    nominal_token_value: &U256,
) -> Result<(), SystemError>
where
    S::IO: IOSubsystemExt,
{
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
    }
}

use core::fmt::Write;
use ruint::aliases::B160;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::AccountDataRequest;
use zk_ee::system::IOSubsystemExt;
use zk_ee::system::Resource;
use zk_ee::system::System;
use zk_ee::system::{EthereumLikeTypes, IOSubsystem};
use zk_ee::system_log;
use zk_ee::utils::Bytes32;

pub const BEACON_ROOTS_ADDRESS: B160 =
    B160::from_limbs([0xb8bb8522d0beac02, 0xd732807ef1319fb7, 0x000f3df6]);

const HISTORY_BUFFER_LENGTH: u64 = 8191;

pub fn eip4788_system_part<S: EthereumLikeTypes>(
    system: &mut System<S>,
    beacon_root: &Bytes32,
) -> Result<(), SystemError>
where
    S::IO: IOSubsystemExt,
{
    let mut resources = S::Resources::FORMAL_INFINITE;

    let props = system.io.read_account_properties(
        ExecutionEnvironmentType::NoEE,
        &mut resources,
        &BEACON_ROOTS_ADDRESS,
        AccountDataRequest::empty()
            .with_observable_bytecode_len()
            .with_is_delegated(),
    )?;

    if !props.is_contract() {
        return Ok(());
    }

    let timestamp = system.get_timestamp();
    let timestamp_idx = timestamp % HISTORY_BUFFER_LENGTH;

    system_log!(
        system,
        "EIP-4788 timestamp = {}, beacon root = {:?}\n",
        timestamp,
        &beacon_root
    );

    let mut timestamp_slot = Bytes32::ZERO;
    timestamp_slot.as_u8_array_mut()[24..32].copy_from_slice(&timestamp_idx.to_be_bytes());

    let mut timestamp_value = Bytes32::ZERO;
    timestamp_value.as_u8_array_mut()[24..32].copy_from_slice(&timestamp.to_be_bytes());

    let mut beacon_root_slot = Bytes32::ZERO;
    beacon_root_slot.as_u8_array_mut()[24..32]
        .copy_from_slice(&(timestamp_idx + HISTORY_BUFFER_LENGTH).to_be_bytes());

    system.io.storage_write::<false>(
        ExecutionEnvironmentType::NoEE,
        &mut resources,
        &BEACON_ROOTS_ADDRESS,
        &timestamp_slot,
        &timestamp_value,
    )?;

    system.io.storage_write::<false>(
        ExecutionEnvironmentType::NoEE,
        &mut resources,
        &BEACON_ROOTS_ADDRESS,
        &beacon_root_slot,
        &beacon_root,
    )?;

    Ok(())
}

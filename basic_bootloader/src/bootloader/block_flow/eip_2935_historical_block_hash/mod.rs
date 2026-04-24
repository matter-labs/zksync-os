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

pub const HISTORY_STORAGE_ADDRESS: B160 =
    B160::from_limbs([0x335B175320002935, 0x27F1C53A10CB7A02, 0x0000F908]);

const HISTORY_SERVE_WINDOW: u64 = 8191;

pub fn eip2935_system_part<S: EthereumLikeTypes>(system: &mut System<S>) -> Result<(), SystemError>
where
    S::IO: IOSubsystemExt,
{
    let mut resources = S::Resources::FORMAL_INFINITE;

    let props = system.io.read_account_properties(
        ExecutionEnvironmentType::NoEE,
        &mut resources,
        &HISTORY_STORAGE_ADDRESS,
        AccountDataRequest::empty()
            .with_observable_bytecode_len()
            .with_is_delegated(),
    )?;

    if !props.is_contract() {
        return Ok(());
    }

    let block_number = system.get_block_number();
    if block_number == 0 {
        return Err(zk_ee::internal_error!("EIP-2935: block number is 0").into());
    }
    let parent_hash = system.get_blockhash(block_number - 1)?;

    system_log!(system, "EIP-2935 parent hash = {:?}\n", &parent_hash);

    let slot_idx = (block_number - 1) % HISTORY_SERVE_WINDOW;
    let mut slot = Bytes32::ZERO;
    slot.as_u8_array_mut()[24..32].copy_from_slice(&slot_idx.to_be_bytes());

    system.io.storage_write::<false>(
        ExecutionEnvironmentType::NoEE,
        &mut resources,
        &HISTORY_STORAGE_ADDRESS,
        &slot,
        &parent_hash,
    )?;

    Ok(())
}

// NOTE: we implement 7002 contract as non-solidity/non-EMV contract as:
// - there is no GAS opcode in the reference bytecode
// - whatever will be the gas supplied to the frame - it'll be sufficient to pop as up to upper bound of elements
// - and to be honest, putting bytecode into execution client is so-so idea, and instead consensus can be instead reached on implementation
// Bytecode for this contract will anyway exist for requests creation in transactions themselves

use core::fmt::Write;
use ruint::aliases::B160;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::AccountDataRequest;
use zk_ee::system::Computational;
use zk_ee::system::IOSubsystemExt;
use zk_ee::system::Resources;
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
    let mut resources = S::Resources::from_native(
        <S::Resources as Resources>::Native::from_computational(u64::MAX),
    );

    let props = resources.with_infinite_ergs(|resources| {
        system.io.read_account_properties(
            ExecutionEnvironmentType::NoEE,
            resources,
            &HISTORY_STORAGE_ADDRESS,
            AccountDataRequest::empty()
                .with_nonce()
                .with_observable_bytecode_len(),
        )
    })?;

    let is_contract = props.nonce.0 == 1 && props.observable_bytecode_len.0 > 0;
    if is_contract == false {
        // fail silently
        return Ok(());
    }

    let block_number = system.get_block_number();
    let parent_hash = system.get_blockhash(block_number - 1)?;

    system_log!(system, "EIP-2935 parent hash = {:?}\n", &parent_hash);

    let slot_idx = (block_number - 1) % HISTORY_SERVE_WINDOW;
    let mut slot = Bytes32::ZERO;
    slot.as_u8_array_mut()[24..32].copy_from_slice(&slot_idx.to_be_bytes());

    resources.with_infinite_ergs(|resources| {
        system.io.storage_write::<false>(
            ExecutionEnvironmentType::NoEE,
            resources,
            &HISTORY_STORAGE_ADDRESS,
            &slot,
            &parent_hash,
        )
    })?;

    Ok(())
}

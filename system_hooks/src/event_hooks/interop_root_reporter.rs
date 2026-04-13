//!
//! Interop root reporter system hook implementation.
//!
use super::super::*;
use ruint::aliases::U256;
use zk_ee::types_config::SystemIOTypesConfig;
use zk_ee::{
    common_structs::interop_root_storage::InteropRoot,
    execution_environment_type::ExecutionEnvironmentType, internal_error,
    system::errors::system::SystemError, system::MAX_EVENT_TOPICS, utils::Bytes32,
};

// InteropRootAdded(uint256,uint256,bytes32[]) - 6b451b8422636e45b93bf7f594fa2c1769d039766c4254a6e7f9c0ee1715cdb0
pub const INTEROP_ROOT_ADDED_EVENT_SIG: [u8; 32] = [
    0x6b, 0x45, 0x1b, 0x84, 0x22, 0x63, 0x6e, 0x45, 0xb9, 0x3b, 0xf7, 0xf5, 0x94, 0xfa, 0x2c, 0x17,
    0x69, 0xd0, 0x39, 0x76, 0x6c, 0x42, 0x54, 0xa6, 0xe7, 0xf9, 0xc0, 0xee, 0x17, 0x15, 0xcd, 0xb0,
];

pub fn interop_root_reporter_event_hook<S: EthereumLikeTypes>(
    topics: &arrayvec::ArrayVec<<S::IOTypes as SystemIOTypesConfig>::EventKey, MAX_EVENT_TOPICS>,
    data: &[u8],
    _caller_ee: u8,
    system: &mut System<S>,
    resources: &mut S::Resources,
) -> Result<(), SystemError>
where
{
    // First, ensure we're capturing the InteropRootAdded event
    if topics.is_empty() || topics[0].as_u8_array() != INTEROP_ROOT_ADDED_EVENT_SIG {
        return Ok(());
    }
    // Internal error if the data supplied doesn't match the expected value
    if data.len() != 96 {
        return Err(internal_error!("Interop root reporter event hook received bad data").into());
    }

    // Parse data
    let offset: u32 = match U256::from_be_slice(&data[..32]).try_into() {
        Ok(offset) => offset,
        Err(_) => {
            return Err(
                internal_error!("Interop root reporter event hook received bad offset").into(),
            );
        }
    };
    // This event is part of the system, but we check it anyways
    if offset != 32 {
        return Err(internal_error!("Interop root reporter event hook received bad offset").into());
    }

    let len: u32 = match U256::from_be_slice(&data[32..64]).try_into() {
        Ok(offset) => offset,
        Err(_) => {
            return Err(
                internal_error!("Interop root reporter event hook received bad length").into(),
            );
        }
    };
    // It should have exactly one side
    if len != 1 {
        return Err(internal_error!("Interop root reporter event hook received bad length").into());
    }
    // Validate topics length
    if topics.len() != 3 {
        return Err(internal_error!("Interop root reporter event hook received bad topics").into());
    }

    let root = Bytes32::from_array(data[64..96].try_into().unwrap());
    let chain_id = U256::from_be_bytes(topics[1].as_u8_array());
    let block_or_batch_number = U256::from_be_bytes(topics[2].as_u8_array());
    system.io.add_interop_root(
        ExecutionEnvironmentType::NoEE,
        resources,
        InteropRoot {
            root,
            block_or_batch_number,
            chain_id,
        },
    )?;

    Ok(())
}

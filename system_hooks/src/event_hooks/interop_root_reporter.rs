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

// InteropRootAdded(uint256,uint256,uint256,bytes32[]) - c5f80b0e9650b87668477e939fc0d8a933964de6b8c4bdc2ae5fdb5723d1f84a
// (chainId and blockNumber indexed; data carries the root's creation timestamp + sides)
pub const INTEROP_ROOT_ADDED_EVENT_SIG: [u8; 32] = [
    0xc5, 0xf8, 0x0b, 0x0e, 0x96, 0x50, 0xb8, 0x76, 0x68, 0x47, 0x7e, 0x93, 0x9f, 0xc0, 0xd8, 0xa9,
    0x33, 0x96, 0x4d, 0xe6, 0xb8, 0xc4, 0xbd, 0xc2, 0xae, 0x5f, 0xdb, 0x57, 0x23, 0xd1, 0xf8, 0x4a,
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
    // Event data is `abi.encode(uint256 timestamp, bytes32[] sides)`:
    // [timestamp][offset of sides = 0x40][sides length = 1][root] = 128 bytes.
    if data.len() != 128 {
        return Err(internal_error!("Interop root reporter event hook received bad data").into());
    }

    // Parse data
    let timestamp = U256::from_be_slice(&data[..32]);
    let offset: u32 = match U256::from_be_slice(&data[32..64]).try_into() {
        Ok(offset) => offset,
        Err(_) => {
            return Err(
                internal_error!("Interop root reporter event hook received bad offset").into(),
            );
        }
    };
    // This event is part of the system, but we check it anyways
    if offset != 64 {
        return Err(internal_error!("Interop root reporter event hook received bad offset").into());
    }

    let len: u32 = match U256::from_be_slice(&data[64..96]).try_into() {
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

    let root = Bytes32::from_array(data[96..128].try_into().unwrap());
    let chain_id = U256::from_be_bytes(topics[1].as_u8_array());
    let block_or_batch_number = U256::from_be_bytes(topics[2].as_u8_array());
    system.io.add_interop_root(
        ExecutionEnvironmentType::NoEE,
        resources,
        InteropRoot {
            root,
            block_or_batch_number,
            chain_id,
            timestamp,
        },
    )?;

    Ok(())
}

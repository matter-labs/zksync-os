//!
//! New settlement layer chain id reporter system hook implementation.
//!
use super::super::*;
use ruint::aliases::U256;
use zk_ee::types_config::SystemIOTypesConfig;
use zk_ee::{
    execution_environment_type::ExecutionEnvironmentType, internal_error,
    storage_types::MAX_EVENT_TOPICS, system::errors::system::SystemError,
};

// SettlementLayerChainIdUpdated(uint256) -  208daf0b9291c1e9a1697737d736630c808045f81f5bc5ae7b8ed740eb5a4d7a
pub const SL_CHAIN_ID_UPDATED_EVENT_SIG: [u8; 32] = [
    0x20, 0x8d, 0xaf, 0x0b, 0x92, 0x91, 0xc1, 0xe9, 0xa1, 0x69, 0x77, 0x37, 0xd7, 0x36, 0x63, 0x0c,
    0x80, 0x80, 0x45, 0xf8, 0x1f, 0x5b, 0xc5, 0xae, 0x7b, 0x8e, 0xd7, 0x40, 0xeb, 0x5a, 0x4d, 0x7a,
];

pub fn system_context_event_hook<S: EthereumLikeTypes>(
    topics: &arrayvec::ArrayVec<<S::IOTypes as SystemIOTypesConfig>::EventKey, MAX_EVENT_TOPICS>,
    data: &[u8],
    caller_ee: u8,
    system: &mut System<S>,
    resources: &mut S::Resources,
) -> Result<(), SystemError>
where
{
    if topics.is_empty() {
        return Ok(());
    }
    // For now, we only capture the SettlementLayerChainIdUpdated event
    if topics[0].as_u8_array() == SL_CHAIN_ID_UPDATED_EVENT_SIG {
        new_sl_chain_id_event_hook(topics, data, caller_ee, system, resources)
    } else {
        Ok(())
    }
}

fn new_sl_chain_id_event_hook<S: EthereumLikeTypes>(
    topics: &arrayvec::ArrayVec<<S::IOTypes as SystemIOTypesConfig>::EventKey, MAX_EVENT_TOPICS>,
    data: &[u8],
    _caller_ee: u8,
    system: &mut System<S>,
    resources: &mut S::Resources,
) -> Result<(), SystemError>
where
{
    // Internal error if the data supplied isn't empty
    if !data.is_empty() {
        return Err(
            internal_error!("New SL chain id reporter event hook received bad data").into(),
        );
    }
    // Same if there's a mismatch in expected topics
    if topics.len() != 2 {
        return Err(
            internal_error!("New SL chain id reporter event hook received bad topics").into(),
        );
    }

    let new_sl_chain_id = U256::from_be_bytes(topics[1].as_u8_array());
    system.io.update_settlement_layer_chain_id(
        ExecutionEnvironmentType::NoEE,
        resources,
        new_sl_chain_id,
    )?;

    Ok(())
}

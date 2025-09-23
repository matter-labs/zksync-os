use std::collections::HashMap;

use crate::test::filler_structure::AccountFillerStructMaybe;
use crate::test::filler_structure::AddressMaybe;
use crate::test::test_structure::block_section::blocks_from_plain_or_wrapped;
use crate::test::test_structure::pre_state::AccountState;
use alloy::primitives::*;
use block_section::BlockSection;
use env_section::EnvSection;
use info_section::InfoSection;
use post_state::PostState;
use pre_state::PreState;
use serde::{de::IgnoredAny, Deserialize};
use transaction_section::TransactionSection;

use super::filler_structure::AccountFillerStruct;

pub mod block_section;
pub mod env_section;
pub mod info_section;
pub mod post_state;
pub mod pre_state;
pub mod transaction_section;

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct StateTestStructure {
    pub _info: InfoSection,
    pub env: EnvSection,
    pub post: HashMap<String, Vec<PostState>>,
    pub pre: PreState,
    pub transaction: TransactionSection,
    config: Option<IgnoredAny>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct BlockchainTestStructure {
    pub network: String,
    config: Option<IgnoredAny>,
    genesis_block_header: Option<IgnoredAny>,
    lastblockhash: Option<IgnoredAny>,
    pub pre: PreState,
    pub post_state: HashMap<AddressMaybe, AccountFillerStructMaybe>,
    #[serde(rename = "genesisRLP")]
    genesis_rlp: Option<IgnoredAny>,
    #[serde(default, deserialize_with = "blocks_from_plain_or_wrapped")]
    pub blocks: Vec<BlockSection>,
    seal_engine: Option<IgnoredAny>,
    #[serde(rename = "_info")]
    pub _info: InfoSection,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TestStructure {
    State(StateTestStructure),
    Blockchain(BlockchainTestStructure),
}

impl TestStructure {
    pub fn state(&self) -> &StateTestStructure {
        match self {
            Self::State(s) => s,
            Self::Blockchain(_) => panic!("Expected state test, found blockchain test"),
        }
    }
}

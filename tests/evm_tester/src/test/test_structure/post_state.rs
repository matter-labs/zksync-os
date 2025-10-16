use alloy::primitives::*;
use serde::Deserialize;
use std::collections::HashMap;

use crate::test::filler_structure::{AccountFillerStructMaybe, AddressMaybe};

#[derive(Debug, Deserialize, Clone)]
pub struct PostStateIndexes {
    pub data: usize,
    pub gas: usize,
    pub value: usize,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PostState {
    pub indexes: PostStateIndexes,
    pub hash: B256,
    pub logs: B256,
    pub txbytes: Bytes,
    pub expect_exception: Option<String>,
    pub state: Option<HashMap<AddressMaybe, AccountFillerStructMaybe>>,
}

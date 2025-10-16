use alloy::primitives::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostStateForCase {
    pub hash: B256,
    pub logs: B256,
    pub txbytes: Bytes,
    pub expect_exception: Option<String>,
}

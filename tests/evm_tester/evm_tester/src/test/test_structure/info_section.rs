use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InfoSection {
    pub comment: Option<String>,
    #[serde(rename = "filling-rpc-server")]
    pub filling_rpc_server: Option<String>,
    #[serde(rename = "filling-tool-version")]
    pub filling_tool_version: Option<String>,
    pub lllcversion: Option<String>,
    pub source: Option<String>,
    pub source_hash: Option<String>,
    pub labels: Option<HashMap<usize, String>>,
    pub hash: Option<String>,
}

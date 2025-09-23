//!
//! The Solidity tests directory file system entity.
//!

use alloy::primitives::*;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;

use super::FSEntity;

///
/// The Solidity tests directory file system entity.
///
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Directory {
    /// Whether the tests directory is enabled.
    pub enabled: bool,
    /// The tests directory comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// The directory entries. Is `None` for files.
    pub entries: BTreeMap<String, FSEntity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_calldatas: Option<Vec<Bytes>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_cases: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_names: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardfork_override: Option<String>,
}

impl Directory {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(entries: BTreeMap<String, FSEntity>) -> Self {
        Self {
            enabled: true,
            entries,
            comment: None,
            skip_calldatas: None,
            skip_cases: None,
            skip_names: None,
            hardfork_override: None,
        }
    }
}

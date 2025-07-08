use super::*;
use alloy::primitives::{Address, Bytes, B256};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct TestJsonResponse<T> {
    pub(crate) result: T,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct AccountProof {
    #[serde(rename = "accountProof")]
    pub(crate) account_proof: Vec<Bytes>,
    pub(crate) address: Address,
    pub(crate) balance: U256,
    #[serde(rename = "codeHash")]
    pub(crate) code_hash: Bytes,
    #[serde(rename = "storageHash")]
    pub(crate) storage_hash: B256,
    pub(crate) nonce: U256,
    #[serde(rename = "storageProof")]
    pub(crate) storage_proof: Vec<StorageProof>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub(crate) struct StorageProof {
    pub(crate) key: B256,
    pub(crate) proof: Vec<Bytes>,
    pub(crate) value: U256,
}

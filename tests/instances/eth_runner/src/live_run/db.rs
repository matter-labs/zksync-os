use crate::block::Block;
use crate::post_check::PostCheckError;
use crate::prestate::{DiffTrace, PrestateTrace};
use crate::receipts::BlockReceipts;
use alloy::primitives::U256;
use anyhow::{Context, Result};
use bincode::config::standard;
use bincode::serde::{decode_from_slice, encode_to_vec};
use serde::{Deserialize, Serialize};
use sled::{Db, Tree};

#[derive(Clone)]
#[allow(dead_code)]
pub struct Database {
    db: Db,
    block_hashes: Tree,
    block_traces: Tree,
    block_status: Tree,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlockStatus {
    Success,
    Error(PostCheckError),
}

// We serialize blocks using json, as the bincode serializer for them is broken
mod as_json_string {
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        let json_str =
            serde_json::to_string(value).map_err(<S::Error as serde::ser::Error>::custom)?;
        serializer.serialize_str(&json_str)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: for<'de2> Deserialize<'de2>,
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        serde_json::from_str(&s).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTraces {
    pub prestate: PrestateTrace,
    pub diff: DiffTrace,
    #[serde(with = "as_json_string")]
    pub block: Block,
    pub receipts: BlockReceipts,
}

impl Database {
    pub fn init(path: String) -> Result<Self> {
        let db = sled::open(path)?;

        let block_hashes = db.open_tree("block_hashes")?;
        let block_traces = db.open_tree("block_traces")?;
        let block_status = db.open_tree("block_status")?;

        Ok(Self {
            db,
            block_hashes,
            block_traces,
            block_status,
        })
    }

    pub fn get_block_hash(&self, block_number: u64) -> Result<Option<U256>> {
        Ok(self
            .block_hashes
            .get(block_number.to_be_bytes())?
            .map(|v| U256::from_le_slice(v.as_ref())))
    }

    pub fn set_block_hash(&self, block_number: u64, hash: U256) -> Result<()> {
        self.block_hashes
            .insert(block_number.to_be_bytes(), hash.to_le_bytes_vec())?;
        self.block_hashes.flush()?;
        Ok(())
    }

    pub fn get_block_traces(&self, block_number: u64) -> Result<Option<BlockTraces>> {
        if let Some(bytes) = self.block_traces.get(block_number.to_be_bytes())? {
            let (status, _) = decode_from_slice::<BlockTraces, _>(&bytes, standard())
                .context("Failed to decode block traces")?;
            Ok(Some(status))
        } else {
            Ok(None)
        }
    }

    pub fn set_block_traces(&self, block_number: u64, traces: &BlockTraces) -> Result<()> {
        let bytes = encode_to_vec(traces, standard()).context("Failed to encode block traces")?;
        self.block_traces
            .insert(block_number.to_be_bytes(), bytes)?;
        self.block_traces.flush()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_block_status(&self, block_number: u64) -> Result<Option<BlockStatus>> {
        if let Some(bytes) = self.block_status.get(block_number.to_be_bytes())? {
            let (status, _) = decode_from_slice::<BlockStatus, _>(&bytes, standard())
                .context("Failed to decode block status")?;
            Ok(Some(status))
        } else {
            Ok(None)
        }
    }

    pub fn set_block_status(&self, block_number: u64, status: BlockStatus) -> Result<()> {
        let bytes = encode_to_vec(&status, standard()).context("Failed to encode block status")?;
        self.block_status
            .insert(block_number.to_be_bytes(), bytes)?;
        self.block_status.flush()?;
        Ok(())
    }
}

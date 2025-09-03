use crate::block::Block;
use crate::calltrace::CallTrace;
use crate::post_check::PostCheckError;
use crate::prestate::{DiffTrace, PrestateTrace};
use crate::receipts::BlockReceipts;
use alloy::primitives::U256;
use anyhow::{Context, Result};
use bincode::config::standard;
use bincode::serde::{decode_from_slice, encode_to_vec};
use csv::Writer;
use serde::{Deserialize, Serialize};
use sled::{Db, Tree};
use std::fs::File;

#[derive(Clone)]
#[allow(dead_code)]
pub struct Database {
    db: Db,
    block_hashes: Tree,
    block_traces: Tree,
    block_status: Tree,
    block_ratios: Tree,
    block_resource_info: Tree,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlockStatus {
    Success,
    Error(PostCheckError),
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ResourceInfo {
    V0 {
        computational_native_used: u64,
        native_used: u64,
        gas_used: u64,
        pubdata_used: u64,
        logs_used: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxId {
    pub block_number: u64,
    pub tx_index: u64,
}

impl ResourceInfo {
    pub fn native_used(&self) -> u64 {
        match self {
            ResourceInfo::V0 { native_used, .. } => *native_used,
        }
    }

    pub fn computational_native_used(&self) -> u64 {
        match self {
            ResourceInfo::V0 {
                computational_native_used,
                ..
            } => *computational_native_used,
        }
    }

    pub fn gas_used(&self) -> u64 {
        match self {
            ResourceInfo::V0 { gas_used, .. } => *gas_used,
        }
    }

    pub fn pubdata_used(&self) -> u64 {
        match self {
            ResourceInfo::V0 { pubdata_used, .. } => *pubdata_used,
        }
    }

    pub fn logs_used(&self) -> u64 {
        match self {
            ResourceInfo::V0 { logs_used, .. } => *logs_used,
        }
    }
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
    pub call: CallTrace,
}

impl Database {
    pub fn init(path: String) -> Result<Self> {
        let db = sled::open(path)?;

        let block_hashes = db.open_tree("block_hashes")?;
        let block_traces = db.open_tree("block_traces")?;
        let block_status = db.open_tree("block_status")?;
        let block_ratios = db.open_tree("block_ratios")?;
        let block_resource_info = db.open_tree("block_resource_info")?;

        Ok(Self {
            db,
            block_hashes,
            block_traces,
            block_status,
            block_ratios,
            block_resource_info,
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

    pub fn set_block_ratio(&self, block_number: u64, ratio: f64) -> Result<()> {
        let bytes = encode_to_vec(ratio, standard()).context("Failed to encode block ratio")?;
        self.block_ratios
            .insert(block_number.to_be_bytes(), bytes)?;
        self.block_ratios.flush()?;
        Ok(())
    }

    pub fn export_block_ratios_to_csv(&self, path: &str) -> Result<()> {
        let mut writer = Writer::from_writer(File::create(path)?);
        writer.write_record(["block_number", "ratio"])?;

        for entry in self.block_ratios.iter() {
            let (key, value) = entry?;
            let block_number = u64::from_be_bytes(key.as_ref().try_into().unwrap());
            let ratio: f64 =
                bincode::serde::decode_from_slice(&value, bincode::config::standard())?.0;
            writer.write_record(&[block_number.to_string(), format!("{ratio:?}")])?;
        }

        writer.flush()?;
        Ok(())
    }

    pub fn iter_failed_block_statuses(&self) -> Result<Vec<(u64, BlockStatus)>> {
        let mut entries = vec![];
        for item in self.block_status.iter() {
            let (k, v) = item?;
            let block_number = u64::from_be_bytes(k.as_ref().try_into()?);
            let status: BlockStatus =
                bincode::serde::decode_from_slice(&v, bincode::config::standard())?.0;
            if status != BlockStatus::Success {
                entries.push((block_number, status));
            }
        }
        Ok(entries)
    }

    fn set_block_resource_info(
        &self,
        block_number: u64,
        tx_index: u64,
        resource_info: ResourceInfo,
    ) -> Result<()> {
        let bytes = encode_to_vec(&resource_info, standard())
            .context("Failed to encode block resource info")?;
        let tx_id = TxId {
            block_number,
            tx_index,
        };
        let id_bytes = encode_to_vec(&tx_id, standard()).context("Failed to encode tx id")?;
        self.block_resource_info.insert(id_bytes, bytes)?;
        self.block_resource_info.flush()?;
        Ok(())
    }

    pub fn set_block_resource_infos(
        &self,
        block_number: u64,
        resource_infos: Vec<ResourceInfo>,
    ) -> Result<()> {
        for (tx_index, resource_info) in resource_infos.into_iter().enumerate() {
            self.set_block_resource_info(block_number, tx_index as u64, resource_info)?;
        }
        Ok(())
    }

    pub fn export_block_resource_info_to_csv(&self, path: &str) -> Result<()> {
        let mut writer = Writer::from_writer(File::create(path)?);
        writer.write_record([
            "block_number",
            "tx_index",
            "native_used",
            "computational_native_used",
            "gas_used",
            "pubdata_used",
            "logs_used",
        ])?;

        for entry in self.block_resource_info.iter() {
            let (key, value) = entry?;
            let tx_id =
                bincode::serde::decode_from_slice::<TxId, _>(&key, bincode::config::standard())?.0;
            let resource_info: ResourceInfo =
                bincode::serde::decode_from_slice(&value, bincode::config::standard())?.0;
            writer.write_record(&[
                tx_id.block_number.to_string(),
                tx_id.tx_index.to_string(),
                format!("{:?}", resource_info.native_used()),
                format!("{:?}", resource_info.computational_native_used()),
                format!("{:?}", resource_info.gas_used()),
                format!("{:?}", resource_info.pubdata_used()),
                format!("{:?}", resource_info.logs_used()),
            ])?;
        }

        writer.flush()?;
        Ok(())
    }
}

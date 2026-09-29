//! The on-disk case file and a serializable snapshot of the block context.
//!
//! `rig::BlockContext` is not `Serialize`, so a small, human-readable snapshot is
//! stored in the case file. The block context a case runs under is fully
//! determined by its `mode`, so replay rebuilds it from `mode` rather than from
//! this snapshot; the snapshot is for triage and for the scenario export.
//!
//! A case file holds the full program (authoritative for replay, since a
//! minimized program corresponds to no seed), the per-case seed (for reference
//! and regeneration), the block-context snapshot, and the outcome.

use anyhow::{Context, Result};
use program_gen::{Mode, Program};
use rig::BlockContext;
use serde::{Deserialize, Serialize};

use crate::classify::Outcome;

/// The fork the lab runs. This repository executes Osaka only; the
/// field is stored so a case file is self-describing.
pub const LAB_FORK: &str = "Osaka";

/// A human-readable snapshot of a `rig::BlockContext`. `U256` values are decimal
/// strings; the coinbase is a debug rendering. Informational only.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockContextRecord {
    pub timestamp: u64,
    pub eip1559_basefee: String,
    pub pubdata_price: String,
    pub native_price: String,
    pub coinbase: String,
    pub gas_limit: u64,
    pub pubdata_limit: u64,
    pub mix_hash: String,
    pub blob_fee: String,
}

impl BlockContextRecord {
    pub fn from_block_context(bc: &BlockContext) -> Self {
        Self {
            timestamp: bc.timestamp,
            eip1559_basefee: bc.eip1559_basefee.to_string(),
            pubdata_price: bc.pubdata_price.to_string(),
            native_price: bc.native_price.to_string(),
            coinbase: format!("{:?}", bc.coinbase),
            gas_limit: bc.gas_limit,
            pubdata_limit: bc.pubdata_limit,
            mix_hash: bc.mix_hash.to_string(),
            blob_fee: bc.blob_fee.to_string(),
        }
    }
}

/// One case, as written under `<out>/findings/` and read back by `replay` and
/// `export`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseRecord {
    /// The master seed of the run this case came from.
    pub master_seed: u64,
    /// The per-case seed. `program_gen::generate(case_seed, fork, mode)`
    /// reproduces `program` for a non-minimized case; a minimized case's program
    /// is stored verbatim and matches no seed.
    pub case_seed: u64,
    /// Index of the case within its run.
    pub case_index: usize,
    /// The execution surface (always `Osaka` in this repository).
    pub fork: String,
    /// The generation mode.
    pub mode: Mode,
    /// The full program: runtime, initcode, calls, notes.
    pub program: Program,
    /// The block context both blocks ran under (informational snapshot).
    pub block_context: BlockContextRecord,
    /// What running the case produced.
    pub outcome: Outcome,
}

impl CaseRecord {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a CaseRecord is always serializable")
    }

    pub fn from_json(text: &str) -> Result<Self> {
        serde_json::from_str(text).context("failed to parse case file")
    }
}

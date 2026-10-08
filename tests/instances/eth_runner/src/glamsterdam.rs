//! Glamsterdam block fixtures: fetching, loading and consistency checks.
//!
//! Glamsterdam changes the block format (EIP-7928 block-level access lists,
//! EIP-7843 slot number in the header), so its fixtures are kept apart from
//! the pre-Glamsterdam ones under `blocks/` and have their own entry points:
//! the `glamsterdam-fetch` / `glamsterdam-check` commands and the
//! `glamsterdam_fixtures_are_consistent` test.
//!
//! A fixture is a directory `<fixture dir>/<block number>/` holding the raw
//! JSON-RPC response bodies of a Geth node, written verbatim so nothing is
//! lost when our types lag behind the node:
//!
//! - `block.json`: `eth_getBlockByNumber(n, true)`, the header with full transactions
//! - `witness.json`: `debug_executionWitness(n)` (`state`, `codes`, `keys`, `headers`)
//! - `block_access_list.json`: `eth_getBlockAccessList(n)`
//! - `raw_header.json`: `debug_getRawHeader(n)`, the header RLP as the node encodes it
//! - `raw_block.json`: `debug_getRawBlock(n)`, the block RLP (header, raw
//!   transactions, ommers, withdrawals and the Glamsterdam body fields)
//! - `raw_receipts.json`: `debug_getRawReceipts(n)`, the EIP-2718 encoded receipts

use alloy::consensus::Header as ConsensusHeader;
use alloy::consensus::Typed2718 as _;
use alloy::eips::eip7928::{compute_block_access_list_hash, AccountChanges};
use alloy::primitives::{keccak256, Bytes};
use alloy::rlp::{Decodable, Encodable};
use alloy::trie::root::ordered_trie_root_encoded;
use alloy_rpc_types_debug::ExecutionWitness;
use anyhow::{bail, ensure, Context};
use rig::log::info;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::live_run::rpc;

/// Fixture directory, relative to this crate (`tests/instances/eth_runner`).
pub const FIXTURE_DIR: &str = "glamsterdam_blocks";

const BLOCK_FILE: &str = "block.json";
const WITNESS_FILE: &str = "witness.json";
const BLOCK_ACCESS_LIST_FILE: &str = "block_access_list.json";
const RAW_HEADER_FILE: &str = "raw_header.json";
const RAW_BLOCK_FILE: &str = "raw_block.json";
const RAW_RECEIPTS_FILE: &str = "raw_receipts.json";

/// `<crate>/glamsterdam_blocks`.
pub fn default_fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_DIR)
}

type RpcBlock = alloy::rpc::types::Block<alloy::rpc::types::Transaction, alloy::rpc::types::Header>;

/// The `result` of a JSON-RPC response; the other envelope fields are ignored.
#[derive(Debug, Deserialize)]
struct RpcResult<T> {
    result: T,
}

/// One stored Glamsterdam block, parsed with the current alloy types.
pub struct GlamsterdamBlock {
    pub block: RpcBlock,
    pub witness: ExecutionWitness,
    pub block_access_list: Vec<AccountChanges>,
    /// Header RLP as encoded by the node (`debug_getRawHeader`).
    pub raw_header: Bytes,
    /// Block RLP as encoded by the node (`debug_getRawBlock`).
    pub raw_block: Bytes,
    /// EIP-2718 encoded receipts, in block order (`debug_getRawReceipts`).
    pub raw_receipts: Vec<Bytes>,
}

/// The top-level items of the raw block RLP, each as its encoding.
pub struct RawBlockBody {
    pub header: Bytes,
    /// Transactions as they are keyed into the transactions trie
    /// (EIP-2718 envelopes: typed ones are RLP strings, legacy ones lists).
    pub transactions: Vec<Bytes>,
    /// Every body field after the transactions: ommers, withdrawals and the
    /// Glamsterdam additions, kept as raw RLP until the format is pinned down.
    pub trailing: Vec<Bytes>,
}

/// Splits one RLP list into the encodings of its items.
fn rlp_list_items(mut data: &[u8]) -> anyhow::Result<Vec<Bytes>> {
    let header = alloy::rlp::Header::decode(&mut data).context("not an RLP item")?;
    ensure!(header.list, "not an RLP list");
    ensure!(
        data.len() == header.payload_length,
        "trailing bytes after the RLP list"
    );
    let mut items = Vec::new();
    while !data.is_empty() {
        let start = data;
        let item = alloy::rlp::Header::decode(&mut data).context("malformed RLP list item")?;
        ensure!(data.len() >= item.payload_length, "truncated RLP list item");
        data = &data[item.payload_length..];
        items.push(Bytes::copy_from_slice(&start[..start.len() - data.len()]));
    }
    Ok(items)
}

impl RawBlockBody {
    pub fn decode(raw_block: &[u8]) -> anyhow::Result<Self> {
        let mut items = rlp_list_items(raw_block).context("the raw block is not an RLP list")?;
        ensure!(
            items.len() >= 2,
            "the raw block has {} fields, expected at least header and transactions",
            items.len()
        );
        let trailing = items.split_off(2);
        // The trie keys the EIP-2718 envelope: typed transactions are RLP
        // strings in the block body, so take their payload; legacy ones are
        // lists, whose encoding is the envelope itself.
        let transactions = rlp_list_items(&items[1])
            .context("the raw block's transactions are not an RLP list")?
            .into_iter()
            .map(|item| {
                let mut payload = item.as_ref();
                let header = alloy::rlp::Header::decode(&mut payload)?;
                Ok(if header.list {
                    item
                } else {
                    Bytes::copy_from_slice(payload)
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Self {
            header: items.swap_remove(0),
            transactions,
            trailing,
        })
    }
}

/// What a consistent fixture contains, for the `glamsterdam-check` report.
#[derive(Debug)]
pub struct FixtureSummary {
    pub number: u64,
    pub slot_number: u64,
    pub gas_used: u64,
    pub transactions: usize,
    /// Body fields of the raw block after the transactions (ommers,
    /// withdrawals, Glamsterdam additions).
    pub raw_body_trailing_fields: usize,
    /// Transaction count per EIP-2718 type.
    pub transaction_types: BTreeMap<u8, usize>,
    pub access_list_accounts: usize,
    pub storage_changes: usize,
    pub storage_reads: usize,
    pub balance_changes: usize,
    pub nonce_changes: usize,
    pub code_changes: usize,
    pub witness_state_nodes: usize,
    pub witness_codes: usize,
    pub witness_keys: usize,
    pub witness_headers: usize,
}

impl std::fmt::Display for FixtureSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "block {} (slot {}): {} txs by type {:?}, {} gas, {} trailing body fields; \
             access list: {} accounts, {} storage changes, {} storage reads, \
             {} balance changes, {} nonce changes, {} code changes; \
             witness: {} state nodes, {} codes, {} keys, {} headers",
            self.number,
            self.slot_number,
            self.transactions,
            self.transaction_types,
            self.gas_used,
            self.raw_body_trailing_fields,
            self.access_list_accounts,
            self.storage_changes,
            self.storage_reads,
            self.balance_changes,
            self.nonce_changes,
            self.code_changes,
            self.witness_state_nodes,
            self.witness_codes,
            self.witness_keys,
            self.witness_headers,
        )
    }
}

fn read_result<T: DeserializeOwned>(dir: &Path, file: &str) -> anyhow::Result<T> {
    let path = dir.join(file);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let mut de = serde_json::Deserializer::from_str(&text);
    de.disable_recursion_limit();
    let RpcResult { result } = RpcResult::<T>::deserialize(&mut de)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(result)
}

impl GlamsterdamBlock {
    /// Loads a fixture directory.
    pub fn from_dir(dir: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            block: read_result(dir, BLOCK_FILE)?,
            witness: read_result(dir, WITNESS_FILE)?,
            block_access_list: read_result(dir, BLOCK_ACCESS_LIST_FILE)?,
            raw_header: read_result(dir, RAW_HEADER_FILE)?,
            raw_block: read_result(dir, RAW_BLOCK_FILE)?,
            raw_receipts: read_result(dir, RAW_RECEIPTS_FILE)?,
        })
    }

    /// Checks that the fixture is internally consistent and that our types
    /// capture it completely:
    ///
    /// - the node's raw header hashes to the block hash, and the header parsed
    ///   from `block.json` re-encodes to exactly that RLP (so no header field,
    ///   in particular none of the Glamsterdam ones, is missing or mis-parsed);
    /// - the header carries `blockAccessListHash` and `slotNumber`;
    /// - the block access list hashes to the header's `blockAccessListHash`;
    /// - every block access index is at most `transactions + 1` (index 0 is the
    ///   pre-transaction system phase, `transactions + 1` the post-transaction one);
    /// - the witness contains the parent header;
    /// - the raw block starts with the raw header and its raw transactions
    ///   hash to `transactionsRoot`, the raw receipts to `receiptsRoot`.
    pub fn check(&self) -> anyhow::Result<FixtureSummary> {
        let header: &ConsensusHeader = &self.block.header.inner;
        let number = header.number;
        let block_hash = self.block.header.hash;

        ensure!(
            keccak256(&self.raw_header) == block_hash,
            "the raw header does not hash to the block hash {block_hash}"
        );
        ensure!(
            header.hash_slow() == block_hash,
            "the header parsed from {BLOCK_FILE} does not hash to the block hash {block_hash}: \
             a header field is missing or mis-parsed"
        );
        let mut encoded = Vec::new();
        header.encode(&mut encoded);
        ensure!(
            encoded == self.raw_header.as_ref(),
            "the header parsed from {BLOCK_FILE} re-encodes differently from the node's raw header"
        );
        let decoded = ConsensusHeader::decode(&mut self.raw_header.as_ref())
            .context("the raw header does not decode as a header")?;
        ensure!(
            &decoded == header,
            "the header decoded from the raw RLP differs from the one parsed from {BLOCK_FILE}"
        );

        let bal_hash = header
            .block_access_list_hash
            .context("the header has no blockAccessListHash: not a Glamsterdam block")?;
        let slot_number = header
            .slot_number
            .context("the header has no slotNumber: not a Glamsterdam block")?;
        let computed_bal_hash = compute_block_access_list_hash(&self.block_access_list);
        ensure!(
            computed_bal_hash == bal_hash,
            "the block access list hashes to {computed_bal_hash}, the header expects {bal_hash}"
        );

        let transactions = self.block.transactions.len();
        let max_index = transactions as u64 + 1;
        let mut storage_changes = 0;
        let mut storage_reads = 0;
        let mut balance_changes = 0;
        let mut nonce_changes = 0;
        let mut code_changes = 0;
        for account in &self.block_access_list {
            storage_reads += account.storage_reads.len();
            balance_changes += account.balance_changes.len();
            nonce_changes += account.nonce_changes.len();
            code_changes += account.code_changes.len();
            let mut indices = Vec::new();
            for slot in &account.storage_changes {
                storage_changes += slot.changes.len();
                indices.extend(slot.changes.iter().map(|c| c.block_access_index.0));
            }
            indices.extend(
                account
                    .balance_changes
                    .iter()
                    .map(|c| c.block_access_index.0),
            );
            indices.extend(account.nonce_changes.iter().map(|c| c.block_access_index.0));
            indices.extend(account.code_changes.iter().map(|c| c.block_access_index.0));
            if let Some(index) = indices.into_iter().find(|index| *index > max_index) {
                bail!(
                    "block access index {index} of {} exceeds {max_index} (the block has {transactions} transactions)",
                    account.address
                );
            }
        }

        let mut parent = None;
        for (i, raw) in self.witness.headers.iter().enumerate() {
            let candidate = ConsensusHeader::decode(&mut raw.as_ref())
                .with_context(|| format!("witness header {i} does not decode as a header"))?;
            if candidate.hash_slow() == header.parent_hash {
                parent = Some(candidate);
            }
        }
        let parent = parent.context("the witness does not contain the parent header")?;
        ensure!(
            parent.number + 1 == number,
            "the parent header in the witness has number {}, expected {}",
            parent.number,
            number - 1
        );

        let body = RawBlockBody::decode(&self.raw_block)?;
        ensure!(
            body.header == self.raw_header,
            "the raw block does not start with the raw header"
        );
        ensure!(
            body.transactions.len() == transactions,
            "the raw block has {} transactions, {BLOCK_FILE} has {transactions}",
            body.transactions.len()
        );
        let transactions_root = ordered_trie_root_encoded(&body.transactions);
        ensure!(
            transactions_root == header.transactions_root,
            "the raw transactions hash to {transactions_root}, the header expects {}",
            header.transactions_root
        );
        ensure!(
            self.raw_receipts.len() == transactions,
            "{} raw receipts for {transactions} transactions",
            self.raw_receipts.len()
        );
        let receipts_root = ordered_trie_root_encoded(&self.raw_receipts);
        ensure!(
            receipts_root == header.receipts_root,
            "the raw receipts hash to {receipts_root}, the header expects {}",
            header.receipts_root
        );

        let mut transaction_types = BTreeMap::new();
        for tx in self.block.transactions.txns() {
            *transaction_types.entry(tx.ty()).or_insert(0) += 1;
        }

        Ok(FixtureSummary {
            number,
            slot_number,
            gas_used: header.gas_used,
            transactions,
            raw_body_trailing_fields: body.trailing.len(),
            transaction_types,
            access_list_accounts: self.block_access_list.len(),
            storage_changes,
            storage_reads,
            balance_changes,
            nonce_changes,
            code_changes,
            witness_state_nodes: self.witness.state.len(),
            witness_codes: self.witness.codes.len(),
            witness_keys: self.witness.keys.len(),
            witness_headers: self.witness.headers.len(),
        })
    }
}

/// One fixture directory, or every fixture directory directly under `path`, sorted
pub fn fixture_dirs(path: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let dirs = if path.join(BLOCK_FILE).exists() {
        vec![path.to_path_buf()]
    } else {
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(path)
            .with_context(|| format!("Failed to list {}", path.display()))?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|dir| dir.join(BLOCK_FILE).exists())
            .collect();
        dirs.sort();
        dirs
    };
    ensure!(!dirs.is_empty(), "no fixtures under {}", path.display());
    Ok(dirs)
}

/// Checks one fixture directory, or every fixture directory directly under
/// `path`, printing a summary per block. Returns how many were checked.
pub fn check_path(path: &Path) -> anyhow::Result<usize> {
    let dirs = fixture_dirs(path)?;
    for dir in &dirs {
        let fixture = GlamsterdamBlock::from_dir(dir)
            .with_context(|| format!("Failed to load fixture {}", dir.display()))?;
        let summary = fixture
            .check()
            .with_context(|| format!("fixture {} is inconsistent", dir.display()))?;
        if let Some(name) = dir
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.parse::<u64>().ok())
        {
            ensure!(
                name == summary.number,
                "fixture {} holds block {}",
                dir.display(),
                summary.number
            );
        }
        println!("{summary}");
    }
    Ok(dirs.len())
}

/// Sends one JSON-RPC request and returns the raw response body after checking
/// that it carries a non-null `result` and no `error`.
fn call_raw(endpoint: &str, method: &str, params: serde_json::Value) -> anyhow::Result<String> {
    let body = rpc::send(
        endpoint,
        json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}),
    )
    .with_context(|| format!("{method} request failed"))?;
    let envelope: serde_json::Value =
        serde_json::from_str(&body).with_context(|| format!("{method} returned invalid JSON"))?;
    if let Some(error) = envelope.get("error") {
        bail!("{method} failed: {error}");
    }
    ensure!(
        envelope
            .get("result")
            .is_some_and(|result| !result.is_null()),
        "{method} returned no result"
    );
    Ok(body)
}

pub fn finalized_block_number(endpoint: &str) -> anyhow::Result<u64> {
    let body = call_raw(
        endpoint,
        "eth_getBlockByNumber",
        json!(["finalized", false]),
    )?;
    let envelope: serde_json::Value = serde_json::from_str(&body)?;
    let number = envelope["result"]["number"]
        .as_str()
        .context("the finalized block has no number")?;
    Ok(u64::from_str_radix(number.trim_start_matches("0x"), 16)?)
}

/// Fetches the fixture of one block into `dir`; files already there are kept.
pub fn fetch_one(endpoint: &str, block_number: u64, dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    let number = format!("0x{block_number:x}");
    let requests = [
        (BLOCK_FILE, "eth_getBlockByNumber", json!([number, true])),
        (WITNESS_FILE, "debug_executionWitness", json!([number])),
        (
            BLOCK_ACCESS_LIST_FILE,
            "eth_getBlockAccessList",
            json!([number]),
        ),
        (RAW_HEADER_FILE, "debug_getRawHeader", json!([number])),
        (RAW_BLOCK_FILE, "debug_getRawBlock", json!([number])),
        (RAW_RECEIPTS_FILE, "debug_getRawReceipts", json!([number])),
    ];
    for (file, method, params) in requests {
        let path = dir.join(file);
        if path.exists() {
            info!("{} already fetched", path.display());
            continue;
        }
        let body = call_raw(endpoint, method, params)
            .with_context(|| format!("Failed to fetch {method} for block {block_number}"))?;
        std::fs::write(&path, body)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        info!("Wrote {}", path.display());
    }
    Ok(())
}

/// Fetches `count` consecutive blocks starting at `first_block` from a Geth
/// node into `<output_dir>/<block>/`, checking each fixture after it is
/// stored. Only finalized blocks are accepted, so the fixtures cannot be
/// invalidated by a reorg.
pub fn fetch(
    endpoint: &str,
    first_block: u64,
    count: u64,
    output_dir: &Path,
) -> anyhow::Result<Vec<u64>> {
    ensure!(count > 0, "count must be positive");
    let last_block = first_block
        .checked_add(count - 1)
        .context("block range overflows")?;
    let finalized = finalized_block_number(endpoint)?;
    ensure!(
        last_block <= finalized,
        "block {last_block} is past the finalized head {finalized}"
    );
    let mut fetched = Vec::with_capacity(count as usize);
    for block_number in first_block..=last_block {
        let dir = output_dir.join(block_number.to_string());
        fetch_one(endpoint, block_number, &dir)?;
        let summary = GlamsterdamBlock::from_dir(&dir)?
            .check()
            .with_context(|| format!("fetched fixture {} is inconsistent", dir.display()))?;
        println!("{summary} -> {}", dir.display());
        fetched.push(block_number);
    }
    Ok(fetched)
}

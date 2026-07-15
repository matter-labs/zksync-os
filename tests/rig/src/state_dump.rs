//! Env-gated, prover-neutral per-block state dump.
//!
//! When the `ZKOS_STATE_DUMP_DIR` env var is set, every block executed through
//! `Chain::run_inner` (i.e. all `Chain::run_block*` entry points, including the
//! path `evm_tester` reaches via `run_block_no_panic`) writes a JSON bundle
//! `<dir>/dump-<counter>-<blocknumber>.json` for external test rigs
//! (equivalence checkers, second provers) that replay ZKsync OS blocks against
//! an independent implementation. The bundle is test-agnostic: all bytecodes
//! are reachable through the preimages of the `pre`/`post` state snapshots.
//! Blocks that fail to execute write no dump.
//!
//! When `ZKOS_STATE_DUMP_DIR` is unset the hook is a strict no-op: `dump_dir()`
//! returns `None`, no snapshot is taken and no file is written.
//!
//! All 32-byte values are lowercase hex WITHOUT a 0x prefix.

use crate::chain::StateDump;
use alloy::hex;
use basic_bootloader::bootloader::block_flow::public_input::{
    BatchOutput, BatchPublicInput, ChainStateCommitment,
};
use basic_bootloader::bootloader::block_header::BlockHeader;
use crypto::MiniDigest;
use forward_system::run::output::BlockOutput;
use ruint::aliases::U256;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use zk_ee::common_structs::da_commitment_scheme::DACommitmentScheme;
use zk_ee::system::metadata::chain_config::ChainConfig;
use zk_ee::utils::Bytes32;

/// ZKsync OS spec tier of this source tree, following the `ZkSpecId`
/// discriminants of the `zksync-os-revm` crate (0 = AtlasV1, 1 = AtlasV2,
/// 2 = AtlasV3, 3 = AtlasV4). This tree is the 0.4.x (AtlasV4) line.
/// Keep in sync with the branch when rebasing the dump hook.
const SPEC_ID: u8 = 3;

/// Protocol version minor this source tree targets. The 0.4.x (AtlasV4) line
/// is the protocol revision after v31 (0.3.x / AtlasV3), i.e. v32. On this
/// line `BatchOutput` carries no chain id (the chain id lives in
/// `ChainConfig`, committed via the public input's `chain_config_hash`) and
/// commits to `interop_roots_rolling_hash` and `settlement_layer_chain_id`.
/// This branch is a draft: keep the constant in sync when rebasing and when
/// the release pins the final protocol version.
const PROTOCOL_VERSION_MINOR: u32 = 32;

/// Process-wide counter making dump file names unique across all blocks (and
/// test threads) of a single test-process run.
static DUMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Directory to write per-block state dumps into, if dumping is enabled.
pub(crate) fn dump_dir() -> Option<PathBuf> {
    std::env::var_os("ZKOS_STATE_DUMP_DIR").map(PathBuf::from)
}

/// Everything captured at the batch-initial-state point of `Chain::run_inner`
/// (the exact state committed via `proof_data`, i.e. after
/// `ensure_account_exists(coinbase)` — the only pre-block state mutation on
/// this path — and before any transaction executes).
pub(crate) struct PreBlockSnapshot {
    /// Target directory (`ZKOS_STATE_DUMP_DIR`).
    pub dir: PathBuf,
    /// EIP-2718 signed tx bytes (RLP variant) or ABI-encoded bytes (L1->L2).
    pub signed_txs: Vec<String>,
    /// Full pre-block flat-storage snapshot (leaves + preimages + root).
    pub pre: StateDump,
    /// Flat-storage tree root before the block.
    pub root_before: Bytes32,
    /// Next free enumeration slot before the block.
    pub next_free_slot_before: u64,
    /// The 256-entry block-hash ring before the block (ring[0] is the oldest
    /// entry, i.e. the hash of block `number - 256`).
    pub block_hashes_before: [U256; 256],
    /// Number of the last block executed before this one (`number - 1`).
    pub previous_block_number: u64,
    /// Timestamp of the last block executed before this one (0 at chain start).
    pub last_block_timestamp_before: u64,
    /// Chain-level execution config the block runs under.
    pub chain_config: ChainConfig,
}

/// One transaction of the block, as submitted plus its execution outcome.
#[derive(serde::Serialize)]
struct TxDump {
    /// EIP-2718 signed tx bytes (RLP variant) or ABI-encoded bytes (L1->L2),
    /// lowercase hex without 0x.
    signed: String,
    /// Gas used by the tx; 0 when `failed` is true.
    gas_used: u64,
    /// True when the native `tx_result` for this tx is `Err`: the tx was in
    /// the block's input list but was not executed (invalid / filtered).
    failed: bool,
}

/// The block environment the STF ran under (mirrors the sealed header values).
#[derive(serde::Serialize)]
struct BlockEnvDump {
    /// Block number.
    number: u64,
    /// Block timestamp (seconds).
    timestamp: u64,
    /// EIP-1559 base fee per gas.
    base_fee: u64,
    /// Block gas limit.
    gas_limit: u64,
    /// Coinbase / fee recipient address, lowercase hex without 0x.
    coinbase: String,
    /// PREVRANDAO / mix hash, lowercase hex without 0x.
    prev_randao: String,
    /// Total gas used by the block.
    gas_used: u64,
}

/// Every field of the native `BlockHeader` struct the STF produced for the
/// block, so the consumer can diff its own header reconstruction field by
/// field. Byte fields are lowercase hex without 0x (`difficulty` as a 32-byte
/// BE word); scalar fields are JSON numbers.
#[derive(serde::Serialize)]
struct NativeHeaderDump {
    parent_hash: String,
    ommers_hash: String,
    beneficiary: String,
    state_root: String,
    transactions_root: String,
    receipts_root: String,
    logs_bloom: String,
    difficulty: String,
    number: u64,
    gas_limit: u64,
    gas_used: u64,
    timestamp: u64,
    extra_data: String,
    mix_hash: String,
    nonce: String,
    base_fee_per_gas: u64,
}

impl NativeHeaderDump {
    fn from_header(header: &BlockHeader) -> Self {
        Self {
            parent_hash: hex::encode(header.parent_hash.as_u8_ref()),
            ommers_hash: hex::encode(header.ommers_hash.as_u8_ref()),
            beneficiary: hex::encode(header.beneficiary.to_be_bytes::<20>()),
            state_root: hex::encode(header.state_root.as_u8_ref()),
            transactions_root: hex::encode(header.transactions_root.as_u8_ref()),
            receipts_root: hex::encode(header.receipts_root.as_u8_ref()),
            logs_bloom: hex::encode(header.logs_bloom),
            difficulty: hex::encode(header.difficulty.to_be_bytes::<32>()),
            number: header.number,
            gas_limit: header.gas_limit,
            gas_used: header.gas_used,
            timestamp: header.timestamp,
            extra_data: hex::encode(header.extra_data.as_slice()),
            mix_hash: hex::encode(header.mix_hash.as_u8_ref()),
            nonce: hex::encode(header.nonce),
            base_fee_per_gas: header.base_fee_per_gas,
        }
    }
}

/// The full per-block JSON bundle.
#[derive(serde::Serialize)]
struct BlockDump {
    /// Chain id (from `ChainConfig`).
    chain_id: u64,
    /// Spec tier of the tree that produced the dump; see [`SPEC_ID`].
    spec_id: u8,
    /// Protocol version minor of the tree that produced the dump; see
    /// [`PROTOCOL_VERSION_MINOR`].
    protocol_version_minor: u32,
    /// `DACommitmentScheme` discriminant the run used.
    da_commitment_scheme: u8,
    /// Block environment the STF ran under.
    block: BlockEnvDump,
    /// Flat-storage tree root before the block, lowercase hex without 0x.
    tree_root_before: String,
    /// Next free enumeration slot (dense leaf count incl. guards) before the block.
    leaf_count_before: u64,
    /// Flat-storage tree root after the block, lowercase hex without 0x.
    tree_root_after: String,
    /// Next free enumeration slot after the block.
    leaf_count_after: u64,
    /// Full pre-block state snapshot (leaves + preimages + root).
    pre: StateDump,
    /// Full post-block state snapshot (leaves + preimages + root).
    post: StateDump,
    /// The block's transactions with per-tx outcomes.
    txs: Vec<TxDump>,
    /// Pubdata of the native prover-input pass, lowercase hex without 0x;
    /// empty when the run was configured without that pass.
    pubdata: String,
    /// Hash of the sealed block header, lowercase hex without 0x.
    block_header_hash: String,
    /// The native `BlockHeader` the STF produced, field by field (must be
    /// consistent with `block_header_hash`; kept separate so a consumer can
    /// detect a divergence between the sealed hash and the header fields).
    native_header: NativeHeaderDump,
    /// The native header's own keccak-of-RLP hash (always equals
    /// `block_header_hash`).
    native_header_hash: String,
    /// blake2s over all 256 pre-block ring entries (each BE-32); the
    /// `last_256_block_hashes_blake` input of the pre-block
    /// `ChainStateCommitment`.
    block_hashes_blake_before: String,
    // Pre-block chain-state inputs of the STF's state commitment, required
    // for mid-chain blocks (block 2+ of multi-block cases) where they differ
    // from the chain-start defaults. Together with tree_root_before,
    // leaf_count_before and block_hashes_blake_before these are ALL the
    // inputs of the native pre-block `ChainStateCommitment`.
    /// Block number before this block (`block.number - 1`).
    block_number_before: u64,
    /// Timestamp of the last block executed before this one (0 at chain
    /// start); the `last_block_timestamp` of the pre-block
    /// `ChainStateCommitment` (== `proof_data.last_block_timestamp`).
    last_block_timestamp_before: u64,
    /// The 255 most recent block hashes before this block (= ring[1..256],
    /// oldest first), each lowercase hex without 0x.
    previous_block_hashes: Vec<String>,
    /// Ring head: hash of block `number - 256` (all-zero for `number <= 256`).
    /// Evicted from `previous_block_hashes` (= ring[1..256]) but still a
    /// valid BLOCKHASH source at exactly the eviction boundary; consumers
    /// cannot derive it from the ring's blake commitment alone.
    block_hash_ring_head: String,
    // Authoritative native ground-truth commitments.
    /// Pre-block `ChainStateCommitment` hash, recomputed from the fields above.
    native_state_before: String,
    /// Post-block `ChainStateCommitment` hash.
    native_state_after: String,
    /// keccak256 hash of the `ChainConfig` the block ran under (the
    /// `chain_config_hash` input of `BatchPublicInput`).
    native_chain_config_hash: String,
    /// keccak256 hash of the native `BatchOutput` of the prover-input pass;
    /// empty when the run was configured without that pass.
    native_batch_output_hash: String,
    /// The STF's `BatchPublicInput` hash (state before/after, chain config
    /// hash, batch output hash); empty when the run was configured without
    /// the prover-input pass.
    native_batch_public_input: String,
    // Chain-config inputs behind native_chain_config_hash.
    /// Whether FRI proof verification is enabled in the `ChainConfig`.
    chain_config_fri: bool,
    /// The `ChainConfig`'s max tx gas limit.
    chain_config_max_tx_gas_limit: u64,
}

/// Assemble and write the JSON bundle for one successfully executed block.
///
/// `batch_output`/`pubdata` are `None` when the run was configured without the
/// native prover-input pass (`do_prover_input_run = false`); in that case
/// `pubdata`, `native_batch_output_hash` and `native_batch_public_input` are
/// emitted as empty strings.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_block_dump(
    snapshot: PreBlockSnapshot,
    post: StateDump,
    root_after: Bytes32,
    next_free_slot_after: u64,
    block_output: &BlockOutput,
    native_header: &BlockHeader,
    batch_output: Option<&BatchOutput>,
    pubdata: Option<&[u8]>,
    da_commitment_scheme: DACommitmentScheme,
) {
    let hdr = &block_output.header;
    let block_number = hdr.number;
    let block_hash = hdr.hash();

    // ---- Authoritative native BatchPublicInput (the STF's 4-field hash) ----
    // state_before: blake2s over all 256 ring entries (each BE-32).
    let mut hasher_before = crypto::blake2s::Blake2s256::new();
    for h in snapshot.block_hashes_before.iter() {
        hasher_before.update(h.to_be_bytes::<32>());
    }
    let last256_before: [u8; 32] = hasher_before.finalize();
    let state_before = ChainStateCommitment {
        state_root: snapshot.root_before,
        next_free_slot: snapshot.next_free_slot_before,
        block_number: snapshot.previous_block_number,
        last_256_block_hashes_blake: last256_before.into(),
        last_block_timestamp: snapshot.last_block_timestamp_before,
    }
    .hash();

    // state_after: blake2s over ring[1..256] (255 entries) then current block hash.
    let mut hasher_after = crypto::blake2s::Blake2s256::new();
    for h in snapshot.block_hashes_before.iter().skip(1) {
        hasher_after.update(h.to_be_bytes::<32>());
    }
    hasher_after.update(block_hash.as_slice());
    let last256_after: [u8; 32] = hasher_after.finalize();
    let state_after = ChainStateCommitment {
        state_root: root_after,
        next_free_slot: next_free_slot_after,
        block_number,
        last_256_block_hashes_blake: last256_after.into(),
        last_block_timestamp: hdr.timestamp,
    }
    .hash();

    let chain_config_hash = snapshot.chain_config.hash();

    let (native_batch_output_hash, native_batch_public_input) = match batch_output {
        Some(batch_output) => {
            let batch_output_hash = batch_output.hash();
            let native_pi = BatchPublicInput {
                state_before: state_before.into(),
                state_after: state_after.into(),
                chain_config_hash: chain_config_hash.into(),
                batch_output: batch_output_hash.into(),
            }
            .hash();
            (hex::encode(batch_output_hash), hex::encode(native_pi))
        }
        None => (String::new(), String::new()),
    };

    // previous 255 block hashes = ring[1..256].
    let previous_block_hashes: Vec<String> = snapshot
        .block_hashes_before
        .iter()
        .skip(1)
        .map(|h| hex::encode(h.to_be_bytes::<32>()))
        .collect();
    let block_hash_ring_head = hex::encode(snapshot.block_hashes_before[0].to_be_bytes::<32>());

    let txs: Vec<TxDump> = snapshot
        .signed_txs
        .into_iter()
        .zip(block_output.tx_results.iter())
        .map(|(signed, result)| TxDump {
            signed,
            gas_used: result.as_ref().map(|output| output.gas_used).unwrap_or(0),
            failed: result.is_err(),
        })
        .collect();

    let dump = BlockDump {
        chain_id: snapshot.chain_config.chain_id(),
        spec_id: SPEC_ID,
        protocol_version_minor: PROTOCOL_VERSION_MINOR,
        da_commitment_scheme: da_commitment_scheme as u8,
        block: BlockEnvDump {
            number: block_number,
            timestamp: hdr.timestamp,
            base_fee: hdr.base_fee_per_gas.unwrap_or(0),
            gas_limit: hdr.gas_limit,
            coinbase: hex::encode(hdr.beneficiary.as_slice()),
            prev_randao: hex::encode(hdr.mix_hash.as_slice()),
            gas_used: hdr.gas_used,
        },
        tree_root_before: hex::encode(snapshot.root_before.as_u8_ref()),
        leaf_count_before: snapshot.next_free_slot_before,
        tree_root_after: hex::encode(root_after.as_u8_ref()),
        leaf_count_after: next_free_slot_after,
        pre: snapshot.pre,
        post,
        txs,
        pubdata: pubdata.map(hex::encode).unwrap_or_default(),
        block_header_hash: hex::encode(block_hash.as_slice()),
        native_header: NativeHeaderDump::from_header(native_header),
        native_header_hash: hex::encode(native_header.hash()),
        block_hashes_blake_before: hex::encode(last256_before),
        block_number_before: snapshot.previous_block_number,
        last_block_timestamp_before: snapshot.last_block_timestamp_before,
        previous_block_hashes,
        block_hash_ring_head,
        native_state_before: hex::encode(state_before),
        native_state_after: hex::encode(state_after),
        native_chain_config_hash: hex::encode(chain_config_hash),
        native_batch_output_hash,
        native_batch_public_input,
        chain_config_fri: snapshot.chain_config.fri_proof_verification_enabled(),
        chain_config_max_tx_gas_limit: snapshot.chain_config.max_tx_gas_limit(),
    };

    let counter = DUMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::fs::create_dir_all(&snapshot.dir).expect("state dump: create ZKOS_STATE_DUMP_DIR");
    let path = snapshot
        .dir
        .join(format!("dump-{counter:06}-{block_number}.json"));
    let json = serde_json::to_string(&dump).expect("state dump: serialize block dump");
    std::fs::write(&path, json).expect("state dump: write block dump");
    log::info!("state dump written to {}", path.display());
}

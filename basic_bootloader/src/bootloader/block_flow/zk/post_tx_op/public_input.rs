use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use ruint::aliases::U256;
use zk_ee::common_structs::da_commitment_scheme::DACommitmentScheme;
use zk_ee::utils::Bytes32;

///
/// Commitment to state that we need to keep between blocks execution:
/// - state commitment(`state_root` and `next_free_slot`)
/// - block number
/// - last 256 block hashes, previous can be "unrolled" from the last, but we commit to 256 for optimization.
/// - last block timestamp, to ensure that block timestamps are not decreasing.
///
/// This commitment(hash of its fields) will be saved on the settlement layer.
/// With proofs, we'll ensure that the values used during block execution correspond to this commitment.
///
#[derive(Debug)]
pub struct ChainStateCommitment {
    pub state_root: Bytes32,
    pub next_free_slot: u64,
    pub block_number: u64,
    pub last_256_block_hashes_blake: Bytes32,
    pub last_block_timestamp: u64,
}

impl ChainStateCommitment {
    ///
    /// Calculate blake2s hash of chain state commitment.
    ///
    /// We are using proving friendly blake2s because this commitment will be generated and opened during proving,
    /// but we don't need to open it on the settlement layer.
    ///
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = crypto::blake2s::Blake2s256::new();
        hasher.update(self.state_root.as_u8_ref());
        hasher.update(&self.next_free_slot.to_be_bytes());
        hasher.update(&self.block_number.to_be_bytes());
        hasher.update(self.last_256_block_hashes_blake.as_u8_ref());
        hasher.update(&self.last_block_timestamp.to_be_bytes());
        hasher.finalize()
    }
}

///
/// Except for proving existence of batch(of blocks) that changes state from one to another, we want to open some info about this batch on the settlement layer:
/// - pubdata: to make sure that it's published and state is recoverable
/// - executed priority ops: to process them on the settlement layer
/// - l2 to l1 logs tree root: to be able to open them on the settlement layer
/// - extra inputs to validate on the settlement layer(timestamp and chain id)
///
#[derive(Debug)]
pub struct BatchOutput {
    /// Chain id used during execution of the blocks.
    pub chain_id: U256,
    /// First block timestamp.
    pub first_block_timestamp: u64,
    /// Last block timestamp.
    pub last_block_timestamp: u64,
    /// DA commitment scheme.
    pub da_commitment_scheme: DACommitmentScheme,
    /// Pubdata commitment.
    pub pubdata_commitment: Bytes32,
    /// Number of l1 -> l2 processed txs in the batch.
    pub number_of_layer_1_txs: U256,
    /// Rolling keccak256 hash of l1 -> l2 txs processed in the batch.
    pub priority_operations_hash: Bytes32,
    /// L2 logs tree root.
    /// Note that it's full root, it's keccak256 of:
    /// - merkle root of l2 -> l1 logs in the batch .
    /// - aggregated root - commitment to logs emitted on chains that settle on the current.
    pub l2_logs_tree_root: Bytes32,
    /// Protocol upgrade tx hash (0 if there wasn't)
    pub upgrade_tx_hash: Bytes32,
    /// Linear keccak256 hash of interop roots
    pub interop_roots_rolling_hash: Bytes32,
    /// Settlement layer chain id.
    pub settlement_layer_chain_id: U256,
}

impl BatchOutput {
    ///
    /// Calculate keccak256 hash of public input
    ///
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Keccak256::new();
        hasher.update(self.chain_id.to_be_bytes::<32>());
        hasher.update(&self.first_block_timestamp.to_be_bytes());
        hasher.update(&self.last_block_timestamp.to_be_bytes());
        // Encode DA commitment scheme as U256 BE
        hasher.update([0u8; 31]);
        hasher.update([self.da_commitment_scheme as u8]);
        hasher.update(self.pubdata_commitment.as_u8_ref());
        hasher.update(self.number_of_layer_1_txs.to_be_bytes::<32>());
        hasher.update(self.priority_operations_hash.as_u8_ref());
        hasher.update(self.l2_logs_tree_root.as_u8_ref());
        hasher.update(self.upgrade_tx_hash.as_u8_ref());
        hasher.update(self.interop_roots_rolling_hash.as_u8_ref());
        hasher.update(self.settlement_layer_chain_id.to_be_bytes::<32>());
        hasher.finalize()
    }
}

#[derive(Debug)]
pub struct BatchPublicInput {
    /// State commitment before the batch.
    /// It should commit for everything needed for trustless execution(state, block number, hashes, etc).
    pub state_before: Bytes32,
    /// State commitment after the batch.
    pub state_after: Bytes32,
    /// Batch output to be opened on the settlement layer, needed to process DA, l1 <> l2 messaging, validate inputs.
    pub batch_output: Bytes32,
}

impl BatchPublicInput {
    ///
    /// Calculate keccak256 hash of public input
    ///
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Keccak256::new();
        hasher.update(self.state_before.as_u8_ref());
        hasher.update(self.state_after.as_u8_ref());
        hasher.update(self.batch_output.as_u8_ref());
        hasher.finalize()
    }
}

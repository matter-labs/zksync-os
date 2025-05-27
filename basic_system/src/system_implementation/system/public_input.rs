use crypto::MiniDigest;
use zk_ee::utils::Bytes32;

///
/// Chain state commitment contains state tree commitment and another chain state info: block number and block hashes.
///
pub struct ChainStateCommitment {
    pub state_root: Bytes32,
    pub next_free_slot: u64,
    pub block_number: u64,
    pub last_256_block_hashes_blake: Bytes32,
}

impl ChainStateCommitment {
    ///
    /// Calculate blake2s hash of chain state commitment.
    ///
    /// We are using balke2s because this commitment will be opened only during proving, but no on l1.
    ///
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = crypto::blake2s::Blake2s256::new();
        hasher.update(self.state_root.as_u8_ref());
        hasher.update(&self.next_free_slot.to_be_bytes());
        hasher.update(&self.block_number.to_be_bytes());
        hasher.update(self.last_256_block_hashes_blake.as_u8_ref());
        hasher.finalize()
    }
}

///
/// Except for proving existence of block that changes state from one to another, we want to open some info about this block on l1:
/// - pubdata: to make sure that it's published and state is recoverable
/// - executed priority ops: to process them on l1
/// - l2 to l1 logs: to send them on l1
/// - upgrade tx: to check it on l1
/// - extra inputs to validate(timestamp and chain id)
///
pub struct BlocksOutput {
    pub chain_id: u64,
    pub first_block_timestamp: u64,
    pub last_block_timestamp: u64,
    /// Linear Blake2s hash of the pubdata
    pub pubdata_hash: Bytes32,
    /// Linear Blake2s hash of executed l1 -> l2 txs hashes
    pub priority_ops_hashes_hash: Bytes32,
    /// Linear Blake2s hash of l2 -> l1 logs hashes
    pub l2_to_l1_logs_hashes_hash: Bytes32,
    /// Protocol upgrade tx hash
    pub upgrade_tx_hash: Bytes32,
}

impl BlocksOutput {
    ///
    /// Calculate blake2s hash of block(s) output.
    ///
    /// We are using balke2s because this commitment will be opened only during proving/aggregation, but no on l1.
    ///
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = crypto::blake2s::Blake2s256::new();
        hasher.update(&self.chain_id.to_be_bytes());
        hasher.update(&self.first_block_timestamp.to_be_bytes());
        hasher.update(&self.last_block_timestamp.to_be_bytes());
        hasher.update(self.pubdata_hash.as_u8_ref());
        hasher.update(self.priority_ops_hashes_hash.as_u8_ref());
        hasher.update(self.l2_to_l1_logs_hashes_hash.as_u8_ref());
        hasher.update(self.upgrade_tx_hash.as_u8_ref());
        hasher.finalize()
    }
}

///
/// Block(s) public input.
/// It can be used for a single block or range of blocks.
///
pub struct BlocksPublicInput {
    pub state_before: ChainStateCommitment,
    pub state_after: ChainStateCommitment,
    pub blocks_output: BlocksOutput,
}

impl BlocksPublicInput {
    ///
    /// Calculate blake2s hash of public input
    ///
    /// We are using balke2s because this public input used for aggregation and will be opened only in aggregation program, not on l1.
    ///
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = crypto::blake2s::Blake2s256::new();
        hasher.update(&self.state_before.hash());
        hasher.update(&self.state_after.hash());
        hasher.update(&self.blocks_output.hash());
        hasher.finalize()
    }
}

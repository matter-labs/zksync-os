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
/// - extra inputs to validate on the settlement layer(timestamp and settlement layer chain id)
///
#[derive(Debug)]
pub struct BatchOutput {
    /// First block timestamp.
    pub first_block_timestamp: u64,
    /// Last block timestamp.
    pub last_block_timestamp: u64,
    /// DA commitment scheme (the commitment *mechanism*: calldata keccak vs blobs).
    pub da_commitment_scheme: DACommitmentScheme,
    /// Pubdata commitment.
    pub pubdata_commitment: Bytes32,
    /// Number of l1 -> l2 processed txs in the batch.
    pub number_of_layer_1_txs: U256,
    /// Number of processed L2 txs in the batch.
    pub number_of_layer_2_txs: U256,
    /// Rolling keccak256 hash of l1 -> l2 txs processed in the batch.
    pub priority_operations_hash: Bytes32,
    /// L2 logs tree root.
    /// Note that it's full root, it's keccak256 of:
    /// - merkle root of l2 -> l1 logs in the batch .
    /// - multichain root - commitment to logs emitted on chains that settle on the current.
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
        hasher.update(&self.first_block_timestamp.to_be_bytes());
        hasher.update(&self.last_block_timestamp.to_be_bytes());
        // Encode DA commitment scheme as U256 BE
        hasher.update([0u8; 31]);
        hasher.update([self.da_commitment_scheme as u8]);
        hasher.update(self.pubdata_commitment.as_u8_ref());
        hasher.update(self.number_of_layer_1_txs.to_be_bytes::<32>());
        hasher.update(self.number_of_layer_2_txs.to_be_bytes::<32>());
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
    /// keccak256 commitment to the chain-level execution rules used during
    /// execution (see [`ChainConfig::hash`]). Committed as a hash so this
    /// layout stays fixed as the config's field set evolves.
    pub chain_config_hash: Bytes32,
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
        hasher.update(self.chain_config_hash.as_u8_ref());
        hasher.update(self.batch_output.as_u8_ref());
        hasher.finalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::hex;
    use zk_ee::common_structs::da_commitment_scheme::PubdataContent;
    use zk_ee::system::metadata::chain_config::{ChainConfig, DEFAULT_MAX_TX_GAS_LIMIT};

    fn sample_chain_config() -> ChainConfig {
        ChainConfig::new(37, false, DEFAULT_MAX_TX_GAS_LIMIT).unwrap()
    }

    fn sample_batch_output() -> BatchOutput {
        BatchOutput {
            first_block_timestamp: 1,
            last_block_timestamp: 2,
            da_commitment_scheme: DACommitmentScheme::BlobsAndPubdataKeccak256,
            pubdata_commitment: Bytes32::ZERO,
            number_of_layer_1_txs: U256::from(3u64),
            number_of_layer_2_txs: U256::from(4u64),
            priority_operations_hash: Bytes32::ZERO,
            l2_logs_tree_root: Bytes32::ZERO,
            upgrade_tx_hash: Bytes32::ZERO,
            interop_roots_rolling_hash: Bytes32::ZERO,
            settlement_layer_chain_id: U256::from(9u64),
        }
    }

    fn sample_public_input(chain_config: ChainConfig) -> BatchPublicInput {
        let batch_output = sample_batch_output();

        BatchPublicInput {
            state_before: Bytes32::ZERO,
            state_after: Bytes32::ZERO,
            chain_config_hash: chain_config.hash().into(),
            batch_output: batch_output.hash().into(),
        }
    }

    #[test]
    fn batch_public_input_hash_commits_to_chain_config() {
        let default_hash = sample_public_input(sample_chain_config()).hash();
        let fri = ChainConfig::new(37, true, DEFAULT_MAX_TX_GAS_LIMIT).unwrap();
        let fri_hash = sample_public_input(fri).hash();

        assert_ne!(default_hash, fri_hash);
    }

    #[test]
    fn batch_output_hash_golden_vector() {
        let default_hash = sample_batch_output().hash();

        assert_eq!(
            default_hash,
            hex!("1c24f398aa0701f9348912ecca748ba93bfb84bfe4f283c16514311419f4f658")
        );
    }

    #[test]
    fn chain_config_hash_golden_vector() {
        // Pins the encoding shared with era-contracts: chain id 37, FRI disabled,
        // default tx gas limit, FullPubdata, L1 filtering and large contracts disabled.
        // Both repos must update together; see docs/system/large_contracts.md.
        assert_eq!(
            sample_chain_config().hash(),
            hex!("a37baa17e852b9c326cea601ab0565fb733b0e0c4590f5a39efc8da5e3ec485c")
        );
    }

    #[test]
    fn batch_public_input_hash_golden_vector() {
        // Shared with era-contracts `TestConstants.sol::PUBLIC_INPUT_HASH_GOLDEN`, checked
        // by `ZKsyncOSPublicInput.t.sol`: the full
        // `keccak256(state_before, state_after, chain_config_hash, batch_output)` for zero state
        // commitments, `sample_chain_config()` and `BATCH_OUTPUT_HASH_GOLDEN`. Both repos must pin the
        // same value; changing the public-input encoding on either side must update both.
        assert_eq!(
            sample_public_input(sample_chain_config()).hash(),
            hex!("115e07747f99e4785e3f1c91d3ff85f05eee22c99004e6a81adc37b1b21495fe")
        );
    }

    #[test]
    fn batch_public_input_hash_flag_golden_vectors() {
        // Shared with era-contracts TestConstants.sol and ZKsyncOSPublicInput.t.sol.
        // The asymmetric cases pin which flag occupies each word.
        for (filtering, large_contracts, config_hash, public_input_hash) in [
            (
                true,
                false,
                hex!("f376cdd377febdda64444c8e0ba5b368ebb3347f597404ffa51a6c19299479c2"),
                hex!("7bb4bea265b944d45623d3e57de083aceb9c7d9f30240c3a917fa187a4444282"),
            ),
            (
                false,
                true,
                hex!("7431a77ce6e77c295d5f7e097382054ac118b993f7b311c765a41f744e99d448"),
                hex!("ac58d82a0990197b31c12ec9cc33b330e5dbda74011eafd37b7e78a5241f0ec7"),
            ),
            (
                true,
                true,
                hex!("d8fab3201bc811330f9f0f5d5a68ed7c30713d11d513da418f93c59f01cd2a0c"),
                hex!("a78ab15446de058e9f871114c4b0333d8ab23ec8959f89b582c33409751aa837"),
            ),
        ] {
            let config = sample_chain_config()
                .with_l1_tx_filtering(filtering)
                .with_large_contracts(large_contracts);
            assert_eq!(config.hash(), config_hash);
            assert_eq!(sample_public_input(config).hash(), public_input_hash);
        }
    }

    #[test]
    fn batch_public_input_hash_commits_to_pubdata_content() {
        let full_pubdata = sample_chain_config(); // `ChainConfig::new` defaults to `PubdataContent::FullPubdata`
        let logs_only = full_pubdata.with_pubdata_content(PubdataContent::LogsOnly);

        assert_ne!(
            sample_public_input(full_pubdata).hash(),
            sample_public_input(logs_only).hash()
        );
    }

    #[test]
    fn batch_public_input_hash_commits_to_l1_tx_filtering() {
        let disabled = sample_chain_config(); // `ChainConfig::new` defaults to L1 tx filtering off
        let enabled = disabled.with_l1_tx_filtering(true);

        assert_ne!(
            sample_public_input(disabled).hash(),
            sample_public_input(enabled).hash()
        );
    }

    #[test]
    fn batch_public_input_hash_commits_to_max_tx_gas_limit() {
        let default_hash = sample_public_input(sample_chain_config()).hash();
        let changed = ChainConfig::new(37, false, DEFAULT_MAX_TX_GAS_LIMIT * 2).unwrap();

        assert_ne!(default_hash, sample_public_input(changed).hash());
    }

    #[test]
    fn batch_public_input_hash_commits_to_chain_id() {
        let base = ChainConfig::new(1, false, DEFAULT_MAX_TX_GAS_LIMIT).unwrap();
        let other = ChainConfig::new(2, false, DEFAULT_MAX_TX_GAS_LIMIT).unwrap();

        assert_ne!(
            sample_public_input(base).hash(),
            sample_public_input(other).hash()
        );
    }

    #[test]
    fn batch_public_input_hash_commits_to_code_size() {
        let config = sample_chain_config();
        let custom = config.with_large_contracts(true);
        assert_ne!(
            sample_public_input(config).hash(),
            sample_public_input(custom).hash()
        );
    }
}

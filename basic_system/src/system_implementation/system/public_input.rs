use crate::system_implementation::system::public_input;
use arrayvec::ArrayVec;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use ruint::aliases::{B160, U256};
use zk_ee::system::logger::Logger;
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
/// Except for proving existence of blocks that changes state from one to another,
/// we want to open some info about these blocks on the settlement layer:
/// - pubdata: to make sure that it's published and state is recoverable
/// - executed priority ops: to process them on l1
/// - l2 to l1 logs: to send them on l1
/// - upgrade tx: to check it on l1
/// - extra inputs to validate(timestamp and chain id)
///
pub struct BlocksOutput {
    /// Chain id used in the blocks.
    pub chain_id: U256,
    /// Timestamp of the first block in the range
    pub first_block_timestamp: u64,
    /// Timestamp of the last block in the range
    pub last_block_timestamp: u64,
    /// Linear Blake2s hash of the pubdata
    pub pubdata_hash: Bytes32,
    /// Linear Blake2s hash of executed l1 -> l2 txs hashes
    pub priority_ops_hashes_hash: Bytes32,
    /// Linear Blake2s hash of l2 -> l1 logs hashes
    pub l2_to_l1_logs_hashes_hash: Bytes32,
    /// Protocol upgrade tx hash (0 if there wasn't)
    pub upgrade_tx_hash: Bytes32,
}

impl BlocksOutput {
    ///
    /// Calculate blake2s hash of block(s) output.
    ///
    /// We are using proving friendly blake2s because this commitment will be calculated during proving/aggregation,
    /// but we don't need to open it on the settlement layer.
    ///
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = crypto::blake2s::Blake2s256::new();
        hasher.update(&self.chain_id.to_be_bytes::<32>());
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
    pub state_before: Bytes32,
    pub state_after: Bytes32,
    pub blocks_output: Bytes32,
}

impl BlocksPublicInput {
    ///
    /// Calculate blake2s hash of public input
    ///
    /// We are using proving friendly blake2s because this commitment will be calculated during proving/aggregation,
    /// but we don't need to open it on the settlement layer.
    ///
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = crypto::blake2s::Blake2s256::new();
        hasher.update(self.state_before.as_u8_ref());
        hasher.update(self.state_after.as_u8_ref());
        hasher.update(self.blocks_output.as_u8_ref());
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
    // TODO(EVM-1081): in future should be commitment scheme
    // pub pubdata_commitment_scheme: DACommitmentScheme,
    pub used_l2_da_validator_address: B160,
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
    /// Rolling hash of all the interop roots included in this batch.
    pub interop_root_rolling_hash: Bytes32,
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
        hasher.update(self.used_l2_da_validator_address.to_be_bytes::<20>());
        hasher.update(self.pubdata_commitment.as_u8_ref());
        hasher.update(self.number_of_layer_1_txs.to_be_bytes::<32>());
        hasher.update(self.priority_operations_hash.as_u8_ref());
        hasher.update(self.l2_logs_tree_root.as_u8_ref());
        hasher.update(self.upgrade_tx_hash.as_u8_ref());
        hasher.update(self.interop_root_rolling_hash.as_u8_ref());
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

///
/// Batch PI builder, it allows to apply blocks info on by one to persist data needed for the batch PI and at the end create it.
///
pub struct BatchPublicInputBuilder {
    is_first_block: bool,
    initial_state_commitment: Option<Bytes32>,
    current_state_commitment: Option<Bytes32>,
    first_block_timestamp: Option<u64>,
    current_block_timestamp: Option<u64>,
    chain_id: Option<U256>,
    pub pubdata_hasher: Keccak256,
    pub logs_storage: ArrayVec<Bytes32, 16384>,
    pub number_of_layer_1_txs: U256,
    pub l1_txs_rolling_hash: Bytes32,
    upgrade_tx_hash: Option<Bytes32>,
}

impl BatchPublicInputBuilder {
    pub fn new() -> Self {
        Self {
            is_first_block: true,
            initial_state_commitment: None,
            current_state_commitment: None,
            first_block_timestamp: None,
            current_block_timestamp: None,
            chain_id: None,
            pubdata_hasher: Keccak256::new(),
            logs_storage: ArrayVec::new(),
            number_of_layer_1_txs: U256::ZERO,
            // keccak256([])
            l1_txs_rolling_hash: Bytes32::from([
                0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c, 0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7,
                0x03, 0xc0, 0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b, 0x7b, 0xfa, 0xd8, 0x04,
                0x5d, 0x85, 0xa4, 0x70,
            ]),
            upgrade_tx_hash: None,
        }
    }

    ///
    /// Apply information about a processed block.
    /// Please note, that pubdata, l2 -> l1 logs, and l1 -> l2 txs commitment should be handled separately using corresponding public fields of this structure.
    ///
    pub fn apply_block(
        &mut self,
        state_commitment_before: Bytes32,
        state_commitment_after: Bytes32,
        block_timestamp: u64,
        chain_id: U256,
        upgrade_tx_hash: Bytes32,
    ) {
        if self.is_first_block {
            self.initial_state_commitment = Some(state_commitment_before);
            self.current_state_commitment = Some(state_commitment_after);
            self.first_block_timestamp = Some(block_timestamp);
            self.current_block_timestamp = Some(block_timestamp);
            self.chain_id = Some(chain_id);
            self.upgrade_tx_hash = Some(upgrade_tx_hash);
            self.is_first_block = false;
        } else {
            assert_eq!(
                self.current_state_commitment.unwrap(),
                state_commitment_before
            );
            self.current_state_commitment = Some(state_commitment_after);
            self.current_block_timestamp = Some(block_timestamp);
            assert_eq!(self.chain_id.unwrap(), chain_id);
            assert!(upgrade_tx_hash.is_zero());
        }
    }

    ///
    /// Create public input for a batch that contains previously added blocks.
    ///
    pub fn into_public_input(self, mut logger: impl Logger) -> BatchPublicInput {
        assert!(!self.is_first_block);

        let mut full_root_hasher = crypto::sha3::Keccak256::new();
        full_root_hasher.update(Self::l2_logs_root(self.logs_storage).as_u8_ref());
        full_root_hasher.update([0u8; 32]); // aggregated root 0 for now
        let full_l2_to_l1_logs_root = full_root_hasher.finalize();

        let mut da_commitment_hasher = crypto::sha3::Keccak256::new();
        da_commitment_hasher.update([0u8; 32]); // we don't have to validate state diffs hash
        da_commitment_hasher.update(self.pubdata_hasher.finalize().as_slice()); // full pubdata keccak
        da_commitment_hasher.update([1u8]); // with calldata we should provide 1 blob
        da_commitment_hasher.update([0u8; 32]); // its hash will be ignored on the settlement layer
        let da_commitment = da_commitment_hasher.finalize();

        let batch_output = public_input::BatchOutput {
            chain_id: self.chain_id.unwrap(),
            first_block_timestamp: self.first_block_timestamp.unwrap(),
            last_block_timestamp: self.current_block_timestamp.unwrap(),
            used_l2_da_validator_address: ruint::aliases::B160::ZERO,
            pubdata_commitment: da_commitment.into(),
            number_of_layer_1_txs: self.number_of_layer_1_txs,
            priority_operations_hash: self.l1_txs_rolling_hash,
            l2_logs_tree_root: full_l2_to_l1_logs_root.into(),
            upgrade_tx_hash: self.upgrade_tx_hash.unwrap(),
            interop_root_rolling_hash: Bytes32::from([0u8; 32]), // for now no interop roots
        };
        let public_input = BatchPublicInput {
            state_before: self.initial_state_commitment.unwrap(),
            state_after: self.current_state_commitment.unwrap(),
            batch_output: batch_output.hash().into(),
        };

        let _ = logger.write_fmt(format_args!(
            "PI calculation: state commitment before {:?}\n",
            self.initial_state_commitment.unwrap()
        ));
        let _ = logger.write_fmt(format_args!(
            "PI calculation: state commitment after {:?}\n",
            self.current_state_commitment.unwrap()
        ));
        let _ = logger.write_fmt(format_args!(
            "PI calculation: batch output {batch_output:?}\n",
        ));
        let _ = logger.write_fmt(format_args!(
            "PI calculation: final batch public input {public_input:?}\n",
        ));

        public_input
    }

    fn l2_logs_root(mut logs: ArrayVec<Bytes32, 16384>) -> Bytes32 {
        const TREE_HEIGHT: usize = 14;
        // keccak256([0; L2_TO_L1_LOG_SERIALIZE_SIZE]), keccak256(keccak256([0; L2_TO_L1_LOG_SERIALIZE_SIZE]) & keccak256([0; L2_TO_L1_LOG_SERIALIZE_SIZE])), ...
        //     0x72abee45b59e344af8a6e520241c4744aff26ed411f4c4b00f8af09adada43ba,
        //     0xc3d03eebfd83049991ea3d3e358b6712e7aa2e2e63dc2d4b438987cec28ac8d0,
        //     0xe3697c7f33c31a9b0f0aeb8542287d0d21e8c4cf82163d0c44c7a98aa11aa111,
        //     0x199cc5812543ddceeddd0fc82807646a4899444240db2c0d2f20c3cceb5f51fa,
        //     0xe4733f281f18ba3ea8775dd62d2fcd84011c8c938f16ea5790fd29a03bf8db89,
        //     0x1798a1fd9c8fbb818c98cff190daa7cc10b6e5ac9716b4a2649f7c2ebcef2272,
        //     0x66d7c5983afe44cf15ea8cf565b34c6c31ff0cb4dd744524f7842b942d08770d,
        //     0xb04e5ee349086985f74b73971ce9dfe76bbed95c84906c5dffd96504e1e5396c,
        //     0xac506ecb5465659b3a927143f6d724f91d8d9c4bdb2463aee111d9aa869874db,
        //     0x124b05ec272cecd7538fdafe53b6628d31188ffb6f345139aac3c3c1fd2e470f,
        //     0xc3be9cbd19304d84cca3d045e06b8db3acd68c304fc9cd4cbffe6d18036cb13f,
        //     0xfef7bd9f889811e59e4076a0174087135f080177302763019adaf531257e3a87,
        //     0xa707d1c62d8be699d34cb74804fdd7b4c568b6c1a821066f126c680d4b83e00b,
        //     0xf6e093070e0389d2e529d60fadb855fdded54976ec50ac709e3a36ceaa64c291,
        //     0x375a5bf909cb02143e3695ca658e0641e739aa590f0004dba93572c44cdb9d2d
        const EMPTY_HASHES: [[u8; 32]; TREE_HEIGHT + 1] = [
            [
                0x72, 0xab, 0xee, 0x45, 0xb5, 0x9e, 0x34, 0x4a, 0xf8, 0xa6, 0xe5, 0x20, 0x24, 0x1c,
                0x47, 0x44, 0xaf, 0xf2, 0x6e, 0xd4, 0x11, 0xf4, 0xc4, 0xb0, 0x0f, 0x8a, 0xf0, 0x9a,
                0xda, 0xda, 0x43, 0xba,
            ],
            [
                0xc3, 0xd0, 0x3e, 0xeb, 0xfd, 0x83, 0x04, 0x99, 0x91, 0xea, 0x3d, 0x3e, 0x35, 0x8b,
                0x67, 0x12, 0xe7, 0xaa, 0x2e, 0x2e, 0x63, 0xdc, 0x2d, 0x4b, 0x43, 0x89, 0x87, 0xce,
                0xc2, 0x8a, 0xc8, 0xd0,
            ],
            [
                0xe3, 0x69, 0x7c, 0x7f, 0x33, 0xc3, 0x1a, 0x9b, 0x0f, 0x0a, 0xeb, 0x85, 0x42, 0x28,
                0x7d, 0x0d, 0x21, 0xe8, 0xc4, 0xcf, 0x82, 0x16, 0x3d, 0x0c, 0x44, 0xc7, 0xa9, 0x8a,
                0xa1, 0x1a, 0xa1, 0x11,
            ],
            [
                0x19, 0x9c, 0xc5, 0x81, 0x25, 0x43, 0xdd, 0xce, 0xed, 0xdd, 0x0f, 0xc8, 0x28, 0x07,
                0x64, 0x6a, 0x48, 0x99, 0x44, 0x42, 0x40, 0xdb, 0x2c, 0x0d, 0x2f, 0x20, 0xc3, 0xcc,
                0xeb, 0x5f, 0x51, 0xfa,
            ],
            [
                0xe4, 0x73, 0x3f, 0x28, 0x1f, 0x18, 0xba, 0x3e, 0xa8, 0x77, 0x5d, 0xd6, 0x2d, 0x2f,
                0xcd, 0x84, 0x01, 0x1c, 0x8c, 0x93, 0x8f, 0x16, 0xea, 0x57, 0x90, 0xfd, 0x29, 0xa0,
                0x3b, 0xf8, 0xdb, 0x89,
            ],
            [
                0x17, 0x98, 0xa1, 0xfd, 0x9c, 0x8f, 0xbb, 0x81, 0x8c, 0x98, 0xcf, 0xf1, 0x90, 0xda,
                0xa7, 0xcc, 0x10, 0xb6, 0xe5, 0xac, 0x97, 0x16, 0xb4, 0xa2, 0x64, 0x9f, 0x7c, 0x2e,
                0xbc, 0xef, 0x22, 0x72,
            ],
            [
                0x66, 0xd7, 0xc5, 0x98, 0x3a, 0xfe, 0x44, 0xcf, 0x15, 0xea, 0x8c, 0xf5, 0x65, 0xb3,
                0x4c, 0x6c, 0x31, 0xff, 0x0c, 0xb4, 0xdd, 0x74, 0x45, 0x24, 0xf7, 0x84, 0x2b, 0x94,
                0x2d, 0x08, 0x77, 0x0d,
            ],
            [
                0xb0, 0x4e, 0x5e, 0xe3, 0x49, 0x08, 0x69, 0x85, 0xf7, 0x4b, 0x73, 0x97, 0x1c, 0xe9,
                0xdf, 0xe7, 0x6b, 0xbe, 0xd9, 0x5c, 0x84, 0x90, 0x6c, 0x5d, 0xff, 0xd9, 0x65, 0x04,
                0xe1, 0xe5, 0x39, 0x6c,
            ],
            [
                0xac, 0x50, 0x6e, 0xcb, 0x54, 0x65, 0x65, 0x9b, 0x3a, 0x92, 0x71, 0x43, 0xf6, 0xd7,
                0x24, 0xf9, 0x1d, 0x8d, 0x9c, 0x4b, 0xdb, 0x24, 0x63, 0xae, 0xe1, 0x11, 0xd9, 0xaa,
                0x86, 0x98, 0x74, 0xdb,
            ],
            [
                0x12, 0x4b, 0x05, 0xec, 0x27, 0x2c, 0xec, 0xd7, 0x53, 0x8f, 0xda, 0xfe, 0x53, 0xb6,
                0x62, 0x8d, 0x31, 0x18, 0x8f, 0xfb, 0x6f, 0x34, 0x51, 0x39, 0xaa, 0xc3, 0xc3, 0xc1,
                0xfd, 0x2e, 0x47, 0x0f,
            ],
            [
                0xc3, 0xbe, 0x9c, 0xbd, 0x19, 0x30, 0x4d, 0x84, 0xcc, 0xa3, 0xd0, 0x45, 0xe0, 0x6b,
                0x8d, 0xb3, 0xac, 0xd6, 0x8c, 0x30, 0x4f, 0xc9, 0xcd, 0x4c, 0xbf, 0xfe, 0x6d, 0x18,
                0x03, 0x6c, 0xb1, 0x3f,
            ],
            [
                0xfe, 0xf7, 0xbd, 0x9f, 0x88, 0x98, 0x11, 0xe5, 0x9e, 0x40, 0x76, 0xa0, 0x17, 0x40,
                0x87, 0x13, 0x5f, 0x08, 0x01, 0x77, 0x30, 0x27, 0x63, 0x01, 0x9a, 0xda, 0xf5, 0x31,
                0x25, 0x7e, 0x3a, 0x87,
            ],
            [
                0xa7, 0x07, 0xd1, 0xc6, 0x2d, 0x8b, 0xe6, 0x99, 0xd3, 0x4c, 0xb7, 0x48, 0x04, 0xfd,
                0xd7, 0xb4, 0xc5, 0x68, 0xb6, 0xc1, 0xa8, 0x21, 0x06, 0x6f, 0x12, 0x6c, 0x68, 0x0d,
                0x4b, 0x83, 0xe0, 0x0b,
            ],
            [
                0xf6, 0xe0, 0x93, 0x07, 0x0e, 0x03, 0x89, 0xd2, 0xe5, 0x29, 0xd6, 0x0f, 0xad, 0xb8,
                0x55, 0xfd, 0xde, 0xd5, 0x49, 0x76, 0xec, 0x50, 0xac, 0x70, 0x9e, 0x3a, 0x36, 0xce,
                0xaa, 0x64, 0xc2, 0x91,
            ],
            [
                0x37, 0x5a, 0x5b, 0xf9, 0x09, 0xcb, 0x02, 0x14, 0x3e, 0x36, 0x95, 0xca, 0x65, 0x8e,
                0x06, 0x41, 0xe7, 0x39, 0xaa, 0x59, 0x0f, 0x00, 0x04, 0xdb, 0xa9, 0x35, 0x72, 0xc4,
                0x4c, 0xdb, 0x9d, 0x2d,
            ],
        ];
        let mut curr_non_default = logs.len();
        let mut hasher = crypto::sha3::Keccak256::new();
        #[allow(clippy::needless_range_loop)]
        for level in 0..TREE_HEIGHT {
            for i in 0..curr_non_default.div_ceil(2) {
                hasher.update(logs[i * 2].as_u8_ref());
                if i * 2 + 1 < curr_non_default {
                    hasher.update(logs[i * 2 + 1].as_u8_ref());
                } else {
                    hasher.update(EMPTY_HASHES[level]);
                }
                logs[i] = hasher.finalize_reset().into();
            }
            curr_non_default = curr_non_default.div_ceil(2);
        }
        if curr_non_default != 0 {
            logs[0]
        } else {
            EMPTY_HASHES[14].into()
        }
    }
}

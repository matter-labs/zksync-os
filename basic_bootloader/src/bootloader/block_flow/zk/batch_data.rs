use super::post_tx_op::da_commitment_generator::DACommitmentGenerator;
use crate::bootloader::block_flow::zk::post_tx_op::calculate_interop_roots_rolling_hash;
use crate::bootloader::block_flow::zk::post_tx_op::public_input::{BatchOutput, BatchPublicInput};
use crate::bootloader::block_flow::{TransactionsRollingKeccakHasher, TxHashesAccumulator};
use arrayvec::ArrayVec;
use crypto::MiniDigest;
use ruint::aliases::U256;
use zk_ee::common_structs::interop_root_storage::InteropRoot;
use zk_ee::common_structs::DACommitmentScheme;
use zk_ee::logger_log;
use zk_ee::oracle::IOOracle;
use zk_ee::system::logger::Logger;
use zk_ee::utils::Bytes32;

///
/// Batch data keeper, it allows applying blocks info one by one to persist data needed for the batch PI.
///
pub struct ZKBatchDataKeeper<A: alloc::alloc::Allocator, O: IOOracle> {
    is_first_block: bool,
    initial_state_commitment: Option<Bytes32>,
    current_state_commitment: Option<Bytes32>,
    first_block_timestamp: Option<u64>,
    current_block_timestamp: Option<u64>,
    chain_id: Option<U256>,
    pub da_commitment_scheme: Option<DACommitmentScheme>,
    pub da_commitment_generator: Option<alloc::boxed::Box<dyn DACommitmentGenerator<O>, A>>,
    pub logs_storage: ArrayVec<Bytes32, 16384>,
    enforced_txs_accumulator: TransactionsRollingKeccakHasher,
    // Includes all transactions
    pub tx_count: U256,
    upgrade_tx_hash: Option<Bytes32>,
    multichain_root: Bytes32,
    interop_roots_rolling_hash: Bytes32,
    settlement_layer_chain_id: Option<U256>,
}

impl<A: alloc::alloc::Allocator, O: IOOracle> ZKBatchDataKeeper<A, O> {
    pub fn new() -> Self {
        Self {
            is_first_block: true,
            initial_state_commitment: None,
            current_state_commitment: None,
            first_block_timestamp: None,
            current_block_timestamp: None,
            chain_id: None,
            da_commitment_generator: None,
            da_commitment_scheme: None,
            logs_storage: ArrayVec::new(),
            // keccak256([])
            enforced_txs_accumulator: TransactionsRollingKeccakHasher::empty(),
            tx_count: U256::ZERO,
            upgrade_tx_hash: None,
            multichain_root: Bytes32::zero(),
            interop_roots_rolling_hash: Bytes32::ZERO,
            settlement_layer_chain_id: None,
        }
    }

    ///
    /// Apply information about a processed block.
    /// Please note, that pubdata, l2 -> l1 logs, and l1 -> l2 txs commitment should be handled separately using corresponding public fields of this structure.
    ///
    pub fn apply_block<'a>(
        &mut self,
        state_commitment_before: Bytes32,
        state_commitment_after: Bytes32,
        block_timestamp: u64,
        chain_id: U256,
        upgrade_tx_hash: Bytes32,
        multichain_root: Bytes32,
        interop_roots: impl Iterator<Item = &'a InteropRoot>,
        settlement_layer_chain_id: U256,
        number_of_txs_in_block: u32,
    ) {
        if self.is_first_block {
            self.initial_state_commitment = Some(state_commitment_before);
            self.current_state_commitment = Some(state_commitment_after);
            self.first_block_timestamp = Some(block_timestamp);
            self.current_block_timestamp = Some(block_timestamp);
            self.chain_id = Some(chain_id);
            self.upgrade_tx_hash = Some(upgrade_tx_hash);
            self.settlement_layer_chain_id = Some(settlement_layer_chain_id);
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
            assert_eq!(
                self.settlement_layer_chain_id,
                Some(settlement_layer_chain_id)
            );
        }
        // we always override multichain root with latest
        self.multichain_root = multichain_root;

        self.tx_count += U256::from(number_of_txs_in_block);

        self.interop_roots_rolling_hash = calculate_interop_roots_rolling_hash(
            self.interop_roots_rolling_hash,
            interop_roots,
            &mut crypto::sha3::Keccak256::new(),
        );
    }

    ///
    /// Returns if the batch has had an upgrade tx
    ///
    pub fn has_upgrade_tx(&self) -> bool {
        self.upgrade_tx_hash
            .is_some_and(|hash| hash != Bytes32::ZERO)
    }

    ///
    /// Create public input for a batch that contains previously added blocks.
    ///
    pub fn into_public_input(self, mut logger: impl Logger, oracle: &mut O) -> BatchPublicInput {
        assert!(!self.is_first_block);
        let has_upgrade_tx = self.has_upgrade_tx();

        let mut chain_batch_root_hasher = crypto::sha3::Keccak256::new();
        chain_batch_root_hasher.update(Self::l2_logs_root(self.logs_storage).as_u8_ref());
        chain_batch_root_hasher.update(self.multichain_root.as_u8_ref());
        let chain_batch_root = chain_batch_root_hasher.finalize();

        let (priority_operations_hash, number_of_layer_1_txs) =
            self.enforced_txs_accumulator.finish();
        let number_of_layer_1_txs = U256::from(number_of_layer_1_txs);
        // Number of L2 transactions can be calculated as:
        // Total txs - l1 txs - upgrade txs
        let mut number_of_layer_2_txs = self.tx_count - number_of_layer_1_txs;
        if has_upgrade_tx {
            number_of_layer_2_txs -= U256::ONE;
        }
        let batch_output = BatchOutput {
            chain_id: self.chain_id.unwrap(),
            first_block_timestamp: self.first_block_timestamp.unwrap(),
            last_block_timestamp: self.current_block_timestamp.unwrap(),
            da_commitment_scheme: self.da_commitment_scheme.unwrap(),
            pubdata_commitment: self.da_commitment_generator.unwrap().finalize(oracle),
            number_of_layer_1_txs,
            number_of_layer_2_txs,
            priority_operations_hash,
            l2_logs_tree_root: chain_batch_root.into(),
            upgrade_tx_hash: self.upgrade_tx_hash.unwrap(),
            interop_roots_rolling_hash: self.interop_roots_rolling_hash,
            settlement_layer_chain_id: self.settlement_layer_chain_id.unwrap(),
        };
        let public_input = BatchPublicInput {
            state_before: self.initial_state_commitment.unwrap(),
            state_after: self.current_state_commitment.unwrap(),
            batch_output: batch_output.hash().into(),
        };

        logger_log!(
            logger,
            "PI calculation: state commitment before {:?}\n",
            self.initial_state_commitment.unwrap()
        );
        logger_log!(
            logger,
            "PI calculation: state commitment after {:?}\n",
            self.current_state_commitment.unwrap()
        );
        logger_log!(logger, "PI calculation: batch output {batch_output:?}\n",);
        logger_log!(
            logger,
            "PI calculation: final batch public input {public_input:?}\n",
        );

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

impl<A: alloc::alloc::Allocator, O: IOOracle> TxHashesAccumulator for ZKBatchDataKeeper<A, O> {
    // not used
    fn empty() -> Self {
        Self::new()
    }

    // used to write l1 txs in tx loop
    fn add_tx_hash(&mut self, tx_hash: &Bytes32) {
        self.enforced_txs_accumulator.add_tx_hash(tx_hash);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::alloc::Global;
    use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
    use zk_ee::system::errors::internal::InternalError;

    struct DummyOracle;

    impl IOOracle for DummyOracle {
        type RawIterator<'a> = core::iter::Empty<usize>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            _query_type: u32,
            _input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            Ok(core::iter::empty())
        }
    }

    #[test]
    fn has_upgrade_tx_is_false_for_none_and_zero_hash() {
        let mut keeper = ZKBatchDataKeeper::<Global, DummyOracle>::new();

        assert!(!keeper.has_upgrade_tx());

        keeper.upgrade_tx_hash = Some(Bytes32::ZERO);
        assert!(!keeper.has_upgrade_tx());
    }

    #[test]
    fn has_upgrade_tx_is_true_for_non_zero_hash() {
        let mut keeper = ZKBatchDataKeeper::<Global, DummyOracle>::new();
        keeper.upgrade_tx_hash = Some(Bytes32::from_byte_fill(1));

        assert!(keeper.has_upgrade_tx());
    }
}

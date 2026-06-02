use super::post_tx_op::da_commitment_generator::DACommitmentGenerator;
use crate::bootloader::block_flow::zk::post_tx_op::calculate_interop_roots_rolling_hash;
use crate::bootloader::block_flow::zk::post_tx_op::public_input::{BatchOutput, BatchPublicInput};
use crate::bootloader::block_flow::{TransactionsRollingKeccakHasher, TxHashesAccumulator};
use arrayvec::ArrayVec;
use crypto::MiniDigest;
use ruint::aliases::U256;
use zk_ee::common_structs::interop_root_storage::InteropRoot;
use zk_ee::common_structs::{
    merkle_root_in_place, DACommitmentScheme, L2_TO_L1_LOG_EMPTY_SUBTREE_HASHES,
};
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
        let empty_hashes = L2_TO_L1_LOG_EMPTY_SUBTREE_HASHES.map(Bytes32::from_array);
        merkle_root_in_place::<crypto::sha3::Keccak256>(&mut logs, &empty_hashes)
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

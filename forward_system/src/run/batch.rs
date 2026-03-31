use crate::run::StorageCommitment;
use crate::run::{NextTxResponse, PreimageSource, ReadStorage, ReadStorageTree, TxSource};
use oracle_provider::MemorySource;
use oracle_provider::OracleQueryProcessor;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use zk_ee::common_structs::da_commitment_scheme::DACommitmentScheme;
use zk_ee::common_structs::ProofData;
use zk_ee::oracle::basic_queries::ZKProofDataQuery;
use zk_ee::oracle::query_ids::BLOCK_METADATA_QUERY_ID;
use zk_ee::oracle::query_ids::DA_COMMITMENT_SCHEME_QUERY_ID;
use zk_ee::oracle::simple_oracle_query::SimpleOracleQuery;
use zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;
use zk_ee::oracle::usize_serialization::UsizeSerializable;
use zksync_os_interface::types::BlockOutput;

use super::BlockContext;

/// Mutable batch pre-state used by the batch prover-input runner.
///
/// The caller provides the state before the first block. After each block run,
/// the runner applies the resulting writes and published preimages so the next
/// block observes the correct pre-state.
pub trait BatchState: ReadStorageTree + PreimageSource {
    /// Advance the batch state to the pre-state of the next block.
    fn apply_block_output(&mut self, block_output: &BlockOutput);
}

#[derive(Debug)]
/// Per-block inputs that are not derived from the evolving batch state.
///
/// `ProofData` is intentionally not included here: the batch runner receives it
/// once for block 1 and then chains it internally between blocks.
pub struct BatchBlockInput<TS> {
    pub block_context: BlockContext,
    pub tx_source: TS,
}

#[derive(Debug, Clone)]
/// Shared cursor used by the oracle responders to route requests to the active block.
pub struct BatchIndex {
    index: Rc<Cell<usize>>,
    len: usize,
}

impl BatchIndex {
    pub fn new(len: usize) -> Self {
        assert!(len > 0, "batch prover input requires at least one block");
        Self {
            index: Rc::new(Cell::new(0)),
            len,
        }
    }

    pub fn current(&self) -> usize {
        assert!(self.index.get() < self.len);
        self.index.get().min(self.len - 1)
    }

    pub fn advance(&self) {
        let current = self.index.get();
        if current + 1 < self.len {
            self.index.set(current + 1);
        }
    }
}

/// Shared mutable wrapper so tree and preimage responders observe the same batch state.
pub struct BatchStateHandle<BS> {
    state: Rc<RefCell<BS>>,
}

impl<BS> BatchStateHandle<BS> {
    pub fn new(state: BS) -> Self {
        Self {
            state: Rc::new(RefCell::new(state)),
        }
    }
}

impl<BS: BatchState> BatchStateHandle<BS> {
    pub fn apply_block_output(&self, block_output: &BlockOutput) {
        self.state.borrow_mut().apply_block_output(block_output);
    }
}

impl<BS> Clone for BatchStateHandle<BS> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}

impl<BS: BatchState> ReadStorage for BatchStateHandle<BS> {
    fn read(&mut self, key: zk_ee::utils::Bytes32) -> Option<zk_ee::utils::Bytes32> {
        self.state.borrow_mut().read(key)
    }
}

impl<BS: BatchState> ReadStorageTree for BatchStateHandle<BS> {
    fn tree_index(&mut self, key: zk_ee::utils::Bytes32) -> Option<u64> {
        self.state.borrow_mut().tree_index(key)
    }

    fn merkle_proof(&mut self, tree_index: u64) -> super::LeafProof {
        self.state.borrow_mut().merkle_proof(tree_index)
    }

    fn prev_tree_index(&mut self, key: zk_ee::utils::Bytes32) -> u64 {
        self.state.borrow_mut().prev_tree_index(key)
    }
}

impl<BS: BatchState> PreimageSource for BatchStateHandle<BS> {
    fn get_preimage(&mut self, hash: zk_ee::utils::Bytes32) -> Option<Vec<u8>> {
        self.state.borrow_mut().get_preimage(hash)
    }
}

#[derive(Debug)]
/// Tx source multiplexer that exposes only the transactions of the active block.
pub struct BatchTxSource<TS> {
    sources: Vec<TS>,
    index: BatchIndex,
}

impl<TS> BatchTxSource<TS> {
    pub fn new(sources: Vec<TS>, index: BatchIndex) -> Self {
        Self { sources, index }
    }

    fn current(&self) -> usize {
        self.index.current()
    }
}

impl<TS: TxSource> TxSource for BatchTxSource<TS> {
    fn get_next_tx(&mut self) -> NextTxResponse {
        let idx = self.current();
        self.sources[idx].get_next_tx()
    }
}

#[derive(Debug)]
/// Serves block metadata for the current block in the batch prover-input run.
pub struct BatchBlockMetadataResponder {
    block_metadata: Vec<BlockContext>,
    index: BatchIndex,
}

impl BatchBlockMetadataResponder {
    pub fn new(block_metadata: Vec<BlockContext>, index: BatchIndex) -> Self {
        Self {
            block_metadata,
            index,
        }
    }
}

impl<M: MemorySource> OracleQueryProcessor<M> for BatchBlockMetadataResponder {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![BLOCK_METADATA_QUERY_ID]
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        query_id == BLOCK_METADATA_QUERY_ID
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        _query: Vec<usize>,
        _memory: &M,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        assert_eq!(query_id, BLOCK_METADATA_QUERY_ID);
        let block_metadata = self.block_metadata[self.index.current()];
        DynUsizeIterator::from_constructor(block_metadata, UsizeSerializable::iter)
    }
}

#[derive(Debug, Clone)]
/// Mutable `ProofData` shared by the proof-data responder across block boundaries.
///
/// The host initializes it with the batch pre-state. After each block, the batch
/// keeper produces the next proof state and this wrapper is updated in place.
pub struct SharedProofData {
    proof_data: Rc<RefCell<ProofData<StorageCommitment>>>,
}

impl SharedProofData {
    pub fn new(proof_data: ProofData<StorageCommitment>) -> Self {
        Self {
            proof_data: Rc::new(RefCell::new(proof_data)),
        }
    }

    pub fn get(&self) -> ProofData<StorageCommitment> {
        *self.proof_data.borrow()
    }

    pub fn set(&self, proof_data: ProofData<StorageCommitment>) {
        *self.proof_data.borrow_mut() = proof_data;
    }
}

#[derive(Debug)]
/// Proof-data responder backed by [`SharedProofData`].
pub struct BatchZKProofDataResponder {
    proof_data: SharedProofData,
}

impl BatchZKProofDataResponder {
    pub fn new(proof_data: SharedProofData) -> Self {
        Self { proof_data }
    }
}

impl<M: MemorySource> OracleQueryProcessor<M> for BatchZKProofDataResponder {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![ZKProofDataQuery::<
            zk_ee::types_config::EthereumIOTypesConfig,
            StorageCommitment,
        >::QUERY_ID]
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        query_id
            == ZKProofDataQuery::<
                zk_ee::types_config::EthereumIOTypesConfig,
                StorageCommitment,
            >::QUERY_ID
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        _query: Vec<usize>,
        _memory: &M,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        assert_eq!(
            query_id,
            ZKProofDataQuery::<zk_ee::types_config::EthereumIOTypesConfig, StorageCommitment>::QUERY_ID
        );
        DynUsizeIterator::from_constructor(self.proof_data.get(), UsizeSerializable::iter)
    }
}

#[derive(Debug)]
/// Batch-wide DA commitment scheme responder.
pub struct BatchDACommitmentSchemeResponder {
    da_commitment_scheme: DACommitmentScheme,
}

impl BatchDACommitmentSchemeResponder {
    pub fn new(da_commitment_scheme: DACommitmentScheme) -> Self {
        Self {
            da_commitment_scheme,
        }
    }
}

impl<M: MemorySource> OracleQueryProcessor<M> for BatchDACommitmentSchemeResponder {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![DA_COMMITMENT_SCHEME_QUERY_ID]
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        query_id == DA_COMMITMENT_SCHEME_QUERY_ID
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        _query: Vec<usize>,
        _memory: &M,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        assert_eq!(query_id, DA_COMMITMENT_SCHEME_QUERY_ID);
        DynUsizeIterator::from_constructor(self.da_commitment_scheme as u8, UsizeSerializable::iter)
    }
}

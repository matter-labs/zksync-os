use super::{InMemoryPreimageSource, InMemoryTree};
use crate::run::convert_alloy::FromAlloy;
use crate::run::BlockOutput;
use crate::run::{BatchState, PreimageSource, ReadStorage, ReadStorageTree};

#[derive(Debug, Clone)]
/// Simple in-memory [`BatchState`] used by batch prover-input tests.
///
/// This mirrors the production contract: one batch-start tree and one
/// preimage source are mutated block by block as outputs are produced.
pub struct InMemoryBatchState<const RANDOMIZED_TREE: bool = false> {
    pub tree: InMemoryTree<RANDOMIZED_TREE>,
    pub preimage_source: InMemoryPreimageSource,
}

impl<const RANDOMIZED_TREE: bool> ReadStorage for InMemoryBatchState<RANDOMIZED_TREE> {
    fn read(&mut self, key: zk_ee::utils::Bytes32) -> Option<zk_ee::utils::Bytes32> {
        self.tree.read(key)
    }
}

impl<const RANDOMIZED_TREE: bool> ReadStorageTree for InMemoryBatchState<RANDOMIZED_TREE> {
    fn tree_index(&mut self, key: zk_ee::utils::Bytes32) -> Option<u64> {
        self.tree.tree_index(key)
    }

    fn merkle_proof(&mut self, tree_index: u64) -> crate::run::LeafProof {
        self.tree.merkle_proof(tree_index)
    }

    fn prev_tree_index(&mut self, key: zk_ee::utils::Bytes32) -> u64 {
        self.tree.prev_tree_index(key)
    }
}

impl<const RANDOMIZED_TREE: bool> PreimageSource for InMemoryBatchState<RANDOMIZED_TREE> {
    fn get_preimage(&mut self, hash: zk_ee::utils::Bytes32) -> Option<Vec<u8>> {
        self.preimage_source.get_preimage(hash)
    }
}

impl<const RANDOMIZED_TREE: bool> BatchState for InMemoryBatchState<RANDOMIZED_TREE> {
    fn apply_block_output(&mut self, block_output: &BlockOutput) {
        for storage_write in &block_output.storage_writes {
            let key = zk_ee::utils::Bytes32::from_alloy(storage_write.key);
            let value = zk_ee::utils::Bytes32::from_alloy(storage_write.value);
            // Mirror the forward runner's visible post-block state so the next
            // block reads from the updated tree snapshot.
            self.tree.cold_storage.insert(key, value);
            self.tree.storage_tree.insert(&key, &value);
        }

        for (hash, preimage) in &block_output.published_preimages {
            self.preimage_source
                .inner
                .insert(zk_ee::utils::Bytes32::from_alloy(*hash), preimage.clone());
        }
    }
}

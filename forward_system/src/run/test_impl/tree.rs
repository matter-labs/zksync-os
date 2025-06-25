use crate::run::{LeafProof, ReadStorage, ReadStorageTree};
use basic_system::system_implementation::flat_storage_model::TestingTree;
use std::alloc::Global;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use zk_ee::utils::Bytes32;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct InMemoryTree<const RANDOMIZED_TREE: bool = false> {
    /// Hash map from a pair of Address, slot into values.
    pub cold_storage: HashMap<Bytes32, Bytes32>,
    pub storage_tree: TestingTree<RANDOMIZED_TREE>,
}

impl<const RANDOMIZED_TREE: bool> InMemoryTree<RANDOMIZED_TREE> {
    pub fn empty() -> Self {
        Self {
            cold_storage: HashMap::new(),
            storage_tree: TestingTree::<{ RANDOMIZED_TREE }>::new_in(Global),
        }
    }
}

impl<const RANDOMIZED_TREE: bool> ReadStorage for InMemoryTree<RANDOMIZED_TREE> {
    fn read(&mut self, key: Bytes32) -> Option<Bytes32> {
        self.cold_storage.get(&key).cloned()
    }
}

impl<const RANDOMIZED_TREE: bool> ReadStorageTree for InMemoryTree<RANDOMIZED_TREE> {
    fn tree_index(&mut self, key: Bytes32) -> Option<u64> {
        Some(self.storage_tree.get_index_for_existing(&key))
    }

    fn merkle_proof(&mut self, tree_index: u64) -> LeafProof {
        self.storage_tree.get_proof_for_position(tree_index)
    }

    fn prev_tree_index(&mut self, key: Bytes32) -> u64 {
        self.storage_tree.get_prev_index(&key)
    }
}


impl<const RANDOMIZED_TREE: bool> ReadStorage for Arc<RwLock<InMemoryTree<RANDOMIZED_TREE>>> {
    fn read(&mut self, key: Bytes32) -> Option<Bytes32> {
        RwLock::read(self).unwrap().cold_storage.get(&key).cloned()
    }
}

impl<const RANDOMIZED_TREE: bool> ReadStorageTree for Arc<RwLock<InMemoryTree<RANDOMIZED_TREE>>> {
    fn tree_index(&mut self, key: Bytes32) -> Option<u64> {
        Some(RwLock::read(self).unwrap().storage_tree.get_index_for_existing(&key))
    }

    fn merkle_proof(&mut self, tree_index: u64) -> LeafProof {
        RwLock::read(self).unwrap().storage_tree.get_proof_for_position(tree_index)
    }

    fn prev_tree_index(&mut self, key: Bytes32) -> u64 {
        RwLock::read(self).unwrap().storage_tree.get_prev_index(&key)
    }
}


impl<const RANDOMIZED_TREE: bool> ReadStorage for Arc<InMemoryTree<RANDOMIZED_TREE>> {
    fn read(&mut self, key: Bytes32) -> Option<Bytes32> {
        self.cold_storage.get(&key).cloned()
    }
}

impl<const RANDOMIZED_TREE: bool> ReadStorageTree for Arc<InMemoryTree<RANDOMIZED_TREE>> {
    fn tree_index(&mut self, key: Bytes32) -> Option<u64> {
        Some(self.storage_tree.get_index_for_existing(&key))
    }

    fn merkle_proof(&mut self, tree_index: u64) -> LeafProof {
        self.storage_tree.get_proof_for_position(tree_index)
    }

    fn prev_tree_index(&mut self, key: Bytes32) -> u64 {
        self.storage_tree.get_prev_index(&key)
    }
}
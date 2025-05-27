use basic_system::system_implementation::io::LeafProof as GenericLeafProof;
use basic_system::system_implementation::io::*;
use zk_ee::utils::Bytes32;

pub type LeafProof = GenericLeafProof<TREE_HEIGHT, Blake2sStorageHasher>;

pub trait ReadStorage: 'static {
    fn read(&mut self, key: Bytes32) -> Option<Bytes32>;
}

pub trait ReadStorageTree: ReadStorage {
    fn tree_index(&mut self, key: Bytes32) -> Option<u64>;

    fn merkle_proof(&mut self, tree_index: u64) -> LeafProof;

    /// Previous tree index must exist, since we add keys with minimal and maximal possible values to the tree by default.
    fn prev_tree_index(&mut self, key: Bytes32) -> u64;
}

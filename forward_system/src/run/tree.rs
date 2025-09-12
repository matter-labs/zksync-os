use basic_system::system_implementation::flat_storage_model::LeafProof as GenericLeafProof;
use basic_system::system_implementation::flat_storage_model::*;
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

impl<T: zksync_os_interface::traits::ReadStorage> ReadStorage for T {
    fn read(&mut self, key: Bytes32) -> Option<Bytes32> {
        self.read(key.as_u8_array().into()).map(|v| v.0.into())
    }
}

impl<T: zksync_os_interface::traits::ReadStorage> ReadStorageTree for T {
    fn tree_index(&mut self, _key: Bytes32) -> Option<u64> {
        unreachable!("VM forward run should not invoke the tree")
    }

    fn merkle_proof(&mut self, _tree_index: u64) -> LeafProof {
        unreachable!("VM forward run should not invoke the tree")
    }

    fn prev_tree_index(&mut self, _key: Bytes32) -> u64 {
        unreachable!("VM forward run should not invoke the tree")
    }
}

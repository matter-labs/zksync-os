use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use forward_system::run::FriProofSidecarSource;
use zk_ee::utils::Bytes32;

/// In-memory mapping from `statement_versioned_hash` to the raw
/// (bincode-serialized) `UnrolledProgramProof` bytes received alongside
/// the `FriProofTx` at admission time.
///
/// This models the server-side handoff: the sidecar is a dumb byte
/// store; all decoding and flattening happens inside
/// `FriProofResponder`.
///
/// The source keeps a shared `lookup_count` that tests can read to
/// assert how many times the bootloader issued a `FRI_PROOF_QUERY_ID`
/// query against it — useful for pinning validator-level dedup
/// behavior. The counter is `Arc<AtomicUsize>` so it survives the
/// `Clone` performed when constructing the forward/proof-mode oracle.
#[derive(Debug, Clone, Default)]
pub struct InMemoryFriProofSidecarSource {
    proofs: BTreeMap<Bytes32, Vec<u8>>,
    lookup_count: Arc<AtomicUsize>,
}

impl InMemoryFriProofSidecarSource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_proof(mut self, statement_versioned_hash: Bytes32, proof_bytes: Vec<u8>) -> Self {
        self.insert(statement_versioned_hash, proof_bytes);
        self
    }

    pub fn insert(&mut self, statement_versioned_hash: Bytes32, proof_bytes: Vec<u8>) {
        self.proofs.insert(statement_versioned_hash, proof_bytes);
    }

    /// Shared handle to the lookup counter. Each call to
    /// `get_proof_bytes` (whether the proof is present or not)
    /// increments this by one. Cloning the source keeps the handle
    /// shared, so tests can pass the source into the rig and still
    /// read the counter afterwards.
    pub fn lookup_counter(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.lookup_count)
    }
}

impl FromIterator<(Bytes32, Vec<u8>)> for InMemoryFriProofSidecarSource {
    fn from_iter<T: IntoIterator<Item = (Bytes32, Vec<u8>)>>(iter: T) -> Self {
        Self {
            proofs: iter.into_iter().collect(),
            lookup_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl FriProofSidecarSource for InMemoryFriProofSidecarSource {
    fn get_proof_bytes(&mut self, statement_versioned_hash: Bytes32) -> Option<Vec<u8>> {
        self.lookup_count.fetch_add(1, Ordering::Relaxed);
        self.proofs.get(&statement_versioned_hash).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_sidecar_source_returns_registered_proof_bytes() {
        let statement_hash = Bytes32::from_array([7u8; 32]);
        let proof_bytes = vec![0xaa, 0xbb, 0xcc];
        let mut source =
            InMemoryFriProofSidecarSource::new().with_proof(statement_hash, proof_bytes.clone());

        assert_eq!(source.get_proof_bytes(statement_hash), Some(proof_bytes));
        assert_eq!(source.get_proof_bytes(Bytes32::from_array([9u8; 32])), None);
    }
}

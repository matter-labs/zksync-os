use std::collections::BTreeMap;

use forward_system::run::make_oracle_for_proofs_and_dumps_with_fri_sidecar;
use forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree};
use forward_system::run::{FriProofSidecarSource, FriVerifierArtifacts};
use zk_ee::common_structs::da_commitment_scheme::DACommitmentScheme;
use zk_ee::common_structs::ProofData;
use zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle;
use zk_ee::utils::Bytes32;
use zksync_os_interface::traits::TxListSource;

use crate::chain::TestingOracleFactory;
use basic_system::system_implementation::flat_storage_model::{FlatStorageCommitment, TREE_HEIGHT};

/// In-memory mapping from `statement_versioned_hash` to the raw
/// (bincode-serialized) `UnrolledProgramProof` bytes received alongside
/// the `FriProofTx` at admission time.
///
/// This models the server-side handoff: the sidecar is a dumb byte
/// store; all decoding and flattening happens inside
/// `FriProofResponder`.
#[derive(Debug, Clone, Default)]
pub struct InMemoryFriProofSidecarSource {
    proofs: BTreeMap<Bytes32, Vec<u8>>,
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
}

impl FromIterator<(Bytes32, Vec<u8>)> for InMemoryFriProofSidecarSource {
    fn from_iter<T: IntoIterator<Item = (Bytes32, Vec<u8>)>>(iter: T) -> Self {
        Self {
            proofs: iter.into_iter().collect(),
        }
    }
}

impl FriProofSidecarSource for InMemoryFriProofSidecarSource {
    fn get_proof_bytes(&mut self, statement_versioned_hash: Bytes32) -> Option<Vec<u8>> {
        self.proofs.get(&statement_versioned_hash).cloned()
    }
}

/// Oracle factory for rig-based tests that need Gateway-style FRI
/// sidecar resolution.
///
/// It mirrors the server/runtime boundary at the point where the
/// bootloader resolves a `statement_versioned_hash` by issuing a
/// `FRI_PROOF_QUERY_ID` oracle query.
#[derive(Debug, Clone, Default)]
pub struct FriProofOracleFactory<const RANDOMIZED_TREE: bool> {
    sidecar_source: InMemoryFriProofSidecarSource,
    verifier_artifacts: Option<FriVerifierArtifacts>,
}

impl<const RANDOMIZED_TREE: bool> FriProofOracleFactory<RANDOMIZED_TREE> {
    pub fn new(sidecar_source: InMemoryFriProofSidecarSource) -> Self {
        Self {
            sidecar_source,
            verifier_artifacts: None,
        }
    }

    pub fn with_verifier_artifacts(mut self, artifacts: FriVerifierArtifacts) -> Self {
        self.verifier_artifacts = Some(artifacts);
        self
    }
}

impl<const RANDOMIZED_TREE: bool> TestingOracleFactory<RANDOMIZED_TREE>
    for FriProofOracleFactory<RANDOMIZED_TREE>
{
    fn create_forward_oracle(
        &self,
        block_metadata: BlockMetadataFromOracle,
        state_tree: InMemoryTree<RANDOMIZED_TREE>,
        preimage_source: InMemoryPreimageSource,
        tx_source: TxListSource,
        proof_data: Option<ProofData<FlatStorageCommitment<TREE_HEIGHT>>>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        add_uart: bool,
        use_native_callable_oracles: bool,
    ) -> oracle_provider::ZkEENonDeterminismSource {
        make_oracle_for_proofs_and_dumps_with_fri_sidecar(
            block_metadata,
            state_tree,
            preimage_source,
            tx_source,
            self.sidecar_source.clone(),
            self.verifier_artifacts.clone(),
            proof_data,
            da_commitment_scheme,
            add_uart,
            use_native_callable_oracles,
        )
    }

    fn create_proof_oracle(
        &self,
        block_metadata: BlockMetadataFromOracle,
        state_tree: InMemoryTree<RANDOMIZED_TREE>,
        preimage_source: InMemoryPreimageSource,
        tx_source: TxListSource,
        proof_data: Option<ProofData<FlatStorageCommitment<TREE_HEIGHT>>>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        add_uart: bool,
        use_native_callable_oracles: bool,
    ) -> oracle_provider::ZkEENonDeterminismSource {
        make_oracle_for_proofs_and_dumps_with_fri_sidecar(
            block_metadata,
            state_tree,
            preimage_source,
            tx_source,
            self.sidecar_source.clone(),
            self.verifier_artifacts.clone(),
            proof_data,
            da_commitment_scheme,
            add_uart,
            use_native_callable_oracles,
        )
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

use std::collections::BTreeMap;

use forward_system::run::make_oracle_for_proofs_and_dumps_with_fri_sidecar;
use forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree};
use forward_system::run::FriProofSidecarSource;
use zk_ee::common_structs::da_commitment_scheme::DACommitmentScheme;
use zk_ee::common_structs::ProofData;
use zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle;
use zk_ee::utils::Bytes32;
use zksync_os_interface::traits::TxListSource;

use crate::chain::TestingOracleFactory;
use basic_system::system_implementation::flat_storage_model::{FlatStorageCommitment, TREE_HEIGHT};

/// In-memory mapping from `statement_versioned_hash` to the exact
/// flattened oracle stream expected by the Airbender unified verifier.
///
/// This models the server-side handoff after the proof sidecar has
/// already been decoded and flattened.
#[derive(Debug, Clone, Default)]
pub struct InMemoryFriProofSidecarSource {
    streams: BTreeMap<Bytes32, Vec<u32>>,
}

impl InMemoryFriProofSidecarSource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_stream(
        mut self,
        statement_versioned_hash: Bytes32,
        oracle_stream: Vec<u32>,
    ) -> Self {
        self.insert(statement_versioned_hash, oracle_stream);
        self
    }

    pub fn insert(&mut self, statement_versioned_hash: Bytes32, oracle_stream: Vec<u32>) {
        self.streams.insert(statement_versioned_hash, oracle_stream);
    }
}

impl FromIterator<(Bytes32, Vec<u32>)> for InMemoryFriProofSidecarSource {
    fn from_iter<T: IntoIterator<Item = (Bytes32, Vec<u32>)>>(iter: T) -> Self {
        Self {
            streams: iter.into_iter().collect(),
        }
    }
}

impl FriProofSidecarSource for InMemoryFriProofSidecarSource {
    fn get_proof_oracle_stream(&mut self, statement_versioned_hash: Bytes32) -> Option<Vec<u32>> {
        self.streams.get(&statement_versioned_hash).cloned()
    }
}

/// Oracle factory for rig-based tests that need Gateway-style FRI sidecar
/// resolution.
///
/// It mirrors the server/runtime boundary at the point where execution
/// resolves a `statement_versioned_hash` into a flattened verifier stream.
#[derive(Debug, Clone, Default)]
pub struct FriProofOracleFactory<const RANDOMIZED_TREE: bool> {
    sidecar_source: InMemoryFriProofSidecarSource,
}

impl<const RANDOMIZED_TREE: bool> FriProofOracleFactory<RANDOMIZED_TREE> {
    pub fn new(sidecar_source: InMemoryFriProofSidecarSource) -> Self {
        Self { sidecar_source }
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
    fn in_memory_sidecar_source_returns_registered_stream() {
        let statement_hash = Bytes32::from_array([7u8; 32]);
        let stream = vec![1u32, 2, 3];
        let mut source =
            InMemoryFriProofSidecarSource::new().with_stream(statement_hash, stream.clone());

        assert_eq!(source.get_proof_oracle_stream(statement_hash), Some(stream));
        assert_eq!(
            source.get_proof_oracle_stream(Bytes32::from_array([9u8; 32])),
            None
        );
    }
}

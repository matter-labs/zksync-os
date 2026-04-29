use super::*;
use crate::run::FriProofSidecarSource;
use execution_utils::setups::CompiledCircuitsSet;
use execution_utils::unified_circuit::flatten_proof_into_responses_for_unified_recursion;
use execution_utils::unrolled::{UnrolledProgramProof, UnrolledProgramSetup};
use zk_ee::oracle::fri_proof_packing::pack_fri_oracle_response;
use zk_ee::oracle::query_ids::FRI_PROOF_QUERY_ID;
use zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;
use zk_ee::utils::Bytes32;

/// Airbender verifier artifacts required to turn a raw
/// `UnrolledProgramProof` byte blob into the flattened oracle word
/// stream the FRI verifier reads.
#[derive(Debug, Clone)]
pub struct FriVerifierArtifacts {
    pub setup: UnrolledProgramSetup,
    pub compiled_layouts: CompiledCircuitsSet,
}

/// Handles `FRI_PROOF_QUERY_ID` oracle queries.
#[derive(Debug, Clone)]
pub struct FriProofResponder<S: FriProofSidecarSource> {
    pub sidecar_source: S,
    pub artifacts: Option<FriVerifierArtifacts>,
}

impl<S: FriProofSidecarSource> OracleQueryProcessor for FriProofResponder<S> {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![FRI_PROOF_QUERY_ID]
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        query_id == FRI_PROOF_QUERY_ID
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        assert_eq!(query_id, FRI_PROOF_QUERY_ID);

        let statement_versioned_hash = Bytes32::from_iter(&mut query.into_iter())
            .expect("must deserialize statement_versioned_hash");

        // Fetch the raw proof bytes for this statement from the sidecar.
        let Some(proof_bytes) = self
            .sidecar_source
            .get_proof_bytes(statement_versioned_hash)
        else {
            log::debug!(
                "FRI sidecar has no entry for statement_versioned_hash={:?}",
                statement_versioned_hash
            );
            return DynUsizeIterator::from_constructor(Vec::new(), |r| r.iter().copied());
        };
        let Some(artifacts) = self.artifacts.as_ref() else {
            log::error!(
                "FRI verifier artifacts not configured on gateway chain — every \
                 FRI_PROOF_TX will be rejected until setup/layout artifacts are \
                 wired into FriProofResponder (statement_versioned_hash={:?})",
                statement_versioned_hash
            );
            return DynUsizeIterator::from_constructor(Vec::new(), |r| r.iter().copied());
        };

        let bincode_config = bincode_v2::config::standard();
        let Ok((proof, _)) = bincode_v2::serde::decode_from_slice::<UnrolledProgramProof, _>(
            &proof_bytes,
            bincode_config,
        ) else {
            log::error!(
                "FRI sidecar bytes failed bincode decode — likely corrupted \
                 sidecar state or encoding mismatch; tx will be rejected as if \
                 the sidecar were missing (statement_versioned_hash={:?}, \
                 proof_bytes_len={})",
                statement_versioned_hash,
                proof_bytes.len()
            );
            return DynUsizeIterator::from_constructor(Vec::new(), |r| r.iter().copied());
        };
        let oracle_stream = flatten_proof_into_responses_for_unified_recursion(
            &proof,
            &artifacts.setup,
            &artifacts.compiled_layouts,
            false,
        );

        let response = pack_fri_oracle_response(&oracle_stream);

        DynUsizeIterator::from_constructor(response, |inner_ref| inner_ref.iter().copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct DummyFriSidecarSource {
        response: Option<Vec<u8>>,
    }

    impl FriProofSidecarSource for DummyFriSidecarSource {
        fn get_proof_bytes(&mut self, _statement_versioned_hash: Bytes32) -> Option<Vec<u8>> {
            self.response.clone()
        }
    }

    fn query() -> Vec<usize> {
        Bytes32::ZERO.iter().collect()
    }

    fn run(responder: &mut FriProofResponder<DummyFriSidecarSource>) -> Vec<usize> {
        responder
            .process_buffered_query(
                FRI_PROOF_QUERY_ID,
                query(),
                &oracle_provider::DummyMemorySource,
            )
            .collect()
    }

    #[test]
    fn missing_artifacts_returns_empty_response() {
        let mut responder = FriProofResponder {
            sidecar_source: DummyFriSidecarSource {
                response: Some(vec![0u8; 64]),
            },
            artifacts: None,
        };
        assert_eq!(run(&mut responder), Vec::<usize>::new());
    }

    #[test]
    fn missing_sidecar_returns_empty_response() {
        let mut responder = FriProofResponder {
            sidecar_source: DummyFriSidecarSource { response: None },
            artifacts: None,
        };
        assert_eq!(run(&mut responder), Vec::<usize>::new());
    }
}

use super::*;
use crate::run::FriProofSidecarSource;
use execution_utils::setups::CompiledCircuitsSet;
use execution_utils::unified_circuit::flatten_proof_into_responses_for_unified_recursion;
use execution_utils::unrolled::{UnrolledProgramProof, UnrolledProgramSetup};
use zk_ee::oracle::query_ids::FRI_PROOF_QUERY_ID;
use zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;
use zk_ee::utils::Bytes32;

/// Airbender verifier artifacts required to turn a raw
/// `UnrolledProgramProof` byte blob into the flattened oracle word
/// stream the bootloader's FRI verifier reads.
#[derive(Debug, Clone)]
pub struct FriVerifierArtifacts {
    pub setup: UnrolledProgramSetup,
    pub compiled_layouts: CompiledCircuitsSet,
}

/// Handles `FRI_PROOF_QUERY_ID` oracle queries. For each query the
/// responder looks up the raw proof bytes from its
/// `FriProofSidecarSource`, bincode-decodes the `UnrolledProgramProof`,
/// and flattens it together with its setup/layout artifacts into the
/// exact `u32` word sequence the Airbender unified verifier reads via
/// `DefaultNonDeterminismSource::read_word()`.
///
/// The sidecar is a dumb byte store; all format knowledge lives here.
///
/// `artifacts` is `None` for callers that never receive FRI proofs
/// (for example non-Gateway chains wired with `NoFriProofSidecar`).
/// When artifacts are missing, the responder returns the same empty
/// response it would return for a missing sidecar entry, which the
/// bootloader host path interprets as `FriProofSidecarMissing`.
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
        // The sidecar is a dumb byte store; decoding and flattening
        // happen here.
        //
        // When the sidecar has no entry for this hash, or when no
        // verifier artifacts have been configured, we return an empty
        // oracle response (zero words). The bootloader host path treats
        // a zero-length response as `FriProofSidecarMissing` and
        // rejects the transaction. This is distinct from a
        // present-but-empty stream which would be [0] (one word: length
        // prefix = 0).
        //
        // The successful response is count-prefixed and payload words
        // are packed in pairs:
        //   [oracle_stream_len, word_0 | (word_1 << 32), ...]
        // The host path unpacks this representation. The CSR path
        // naturally sees the low/high halves as consecutive verifier
        // words.
        let Some(proof_bytes) = self
            .sidecar_source
            .get_proof_bytes(statement_versioned_hash)
        else {
            return DynUsizeIterator::from_constructor(Vec::new(), |r| r.iter().copied());
        };
        let Some(artifacts) = self.artifacts.as_ref() else {
            return DynUsizeIterator::from_constructor(Vec::new(), |r| r.iter().copied());
        };

        let bincode_config = bincode_v2::config::standard();
        let (proof, _): (UnrolledProgramProof, usize) =
            bincode_v2::serde::decode_from_slice(&proof_bytes, bincode_config)
                .expect("must decode UnrolledProgramProof from sidecar bytes");
        let oracle_stream = flatten_proof_into_responses_for_unified_recursion(
            &proof,
            &artifacts.setup,
            &artifacts.compiled_layouts,
            false,
        );

        let mut response = Vec::with_capacity(1 + oracle_stream.len().div_ceil(2));
        response.push(oracle_stream.len());
        for pair in oracle_stream.chunks(2) {
            let low = pair[0] as usize;
            let high = pair.get(1).copied().unwrap_or(0) as usize;
            response.push(low | (high << 32));
        }

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
        // Sidecar has bytes but no verifier artifacts are configured.
        // The bootloader host path must see this as a missing sidecar
        // and reject the transaction rather than attempt to decode.
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
        // A missing sidecar must return zero oracle words so the host
        // path triggers FriProofSidecarMissing, not a silent empty proof.
        let mut responder = FriProofResponder {
            sidecar_source: DummyFriSidecarSource { response: None },
            artifacts: None,
        };
        assert_eq!(run(&mut responder), Vec::<usize>::new());
    }
}

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
        // All three failure modes below produce an empty oracle response
        // so the bootloader rejects the tx with `FriProofSidecarMissing`.
        // The interface cannot distinguish them, but each has a very
        // different operational meaning — we log them separately so
        // operators can tell "user submitted bad hash" from "sequencer
        // is misconfigured".
        //
        // (a) No sidecar entry — expected transient case (user's proof
        //     upload raced tx gossip, or user submitted a hash they
        //     never proved). High volume possible; debug level avoids
        //     log flood.
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
        // (b) No verifier artifacts configured — sequencer misconfig on
        //     a gateway chain. EVERY FRI tx will be rejected until this
        //     is fixed, so this is an actionable alert, not a one-off.
        let Some(artifacts) = self.artifacts.as_ref() else {
            log::error!(
                "FRI verifier artifacts not configured on gateway chain — every \
                 FRI_PROOF_TX will be rejected until setup/layout artifacts are \
                 wired into FriProofResponder (statement_versioned_hash={:?})",
                statement_versioned_hash
            );
            return DynUsizeIterator::from_constructor(Vec::new(), |r| r.iter().copied());
        };

        // (c) Sidecar bytes are not a valid bincode-encoded
        //     `UnrolledProgramProof`. Production sidecars are populated
        //     only after the admission layer decoded+verified the proof
        //     against these same artifacts, so a decode failure here
        //     means corrupted local sidecar state (disk bitrot, a bug
        //     that wrote wrong bytes, an upgrade that changed the
        //     encoding). Neither case is user-driven; surface it as an
        //     error so ops can correlate with the sidecar hash.
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

        // Pack into the shared FRI oracle response layout
        // (see `zk_ee::oracle::fri_proof_packing`): length prefix
        // followed by two verifier words per payload usize.
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

use super::*;
use crate::run::fri_proof_decode::decode_and_flatten_proof;
use crate::run::FriProofSidecarSource;
use execution_utils::setups::CompiledCircuitsSet;
use execution_utils::unrolled::UnrolledProgramSetup;
use std::sync::Arc;
use zk_ee::oracle::query_ids::FRI_PROOF_QUERY_ID;
use zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;
use zk_ee::utils::usize_rw::ReadIterWrapper;
use zk_ee::utils::Bytes32;

#[cfg(not(all(target_pointer_width = "64", target_endian = "little")))]
compile_error!("FriProofResponder host packing requires a 64-bit little-endian host target");

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
    pub artifacts: Option<Arc<FriVerifierArtifacts>>,
}

/// Reinterpret a `Vec<u32>` response stream as the host's `usize`
/// stream by reading the underlying bytes as little-endian u64s.
fn u32_words_as_host_usize_stream(
    response_words: Vec<u32>,
) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
    DynUsizeIterator::from_constructor(response_words, |inner_ref| {
        let bytes = unsafe {
            core::slice::from_raw_parts(
                inner_ref.as_ptr().cast::<u8>(),
                inner_ref.len() * core::mem::size_of::<u32>(),
            )
        };
        ReadIterWrapper::from(bytes.iter().copied())
    })
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

        let Some(oracle_stream) = decode_and_flatten_proof(&proof_bytes, artifacts) else {
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

        let oracle_stream_len =
            u32::try_from(oracle_stream.len()).expect("FRI oracle stream length fits into u32");

        let mut response_words = Vec::with_capacity(1 + oracle_stream.len());
        response_words.push(oracle_stream_len);
        response_words.extend(oracle_stream);

        u32_words_as_host_usize_stream(response_words)
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

    #[test]
    fn u32_words_are_exposed_as_standard_host_usize_words() {
        let response = u32_words_as_host_usize_stream(vec![4, 11, 22, 33, 44]).collect::<Vec<_>>();

        assert_eq!(
            response,
            vec![
                4usize | ((11usize) << 32),
                22usize | ((33usize) << 32),
                44usize,
            ]
        );
    }

    #[test]
    fn odd_total_u32_word_count_has_no_trailing_padding_word() {
        let response = u32_words_as_host_usize_stream(vec![3, 11, 22, 33]).collect::<Vec<_>>();

        assert_eq!(
            response,
            vec![3usize | ((11usize) << 32), 22usize | ((33usize) << 32),]
        );
    }
}

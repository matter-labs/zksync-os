use super::*;
use crate::run::FriProofSidecarSource;
use zk_ee::oracle::query_ids::FRI_PROOF_QUERY_ID;
use zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;
use zk_ee::utils::Bytes32;

/// Handles `FRI_PROOF_QUERY_ID` oracle queries by routing them to a
/// `FriProofSidecarSource`. The bootloader's FRI tx handler issues one
/// such query per `statement_versioned_hash` before calling the
/// Airbender unified verifier; the responder returns the flattened
/// oracle stream the verifier will then read word-by-word via
/// `DefaultNonDeterminismSource::read_word()`.
///
/// Keeping the responder stateless means the sidecar source is free to
/// be backed by any storage the server / test rig prefers.
#[derive(Debug, Clone)]
pub struct FriProofResponder<S: FriProofSidecarSource> {
    pub sidecar_source: S,
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

        // Fetch the pre-flattened oracle stream for this statement from
        // the sidecar. The sidecar source is responsible for decoding
        // the raw proof bytes and running the flattener; we just
        // forward the result.
        //
        // When the sidecar is absent (`None`) we return an empty oracle
        // response (zero words). The bootloader host path treats a
        // zero-length response as `FriProofSidecarMissing` and rejects
        // the transaction. This is distinct from a present-but-empty
        // stream which would be [0] (one word: length prefix = 0).
        //
        // When the sidecar is present the response is count-prefixed and
        // payload words are packed in pairs:
        //   [oracle_stream_len, word_0 | (word_1 << 32), ...]
        // The host path unpacks this representation. The CSR path naturally
        // sees the low/high halves as consecutive verifier words.
        let Some(oracle_stream) = self
            .sidecar_source
            .get_proof_oracle_stream(statement_versioned_hash)
        else {
            return DynUsizeIterator::from_constructor(Vec::new(), |r| r.iter().copied());
        };
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
        response: Option<Vec<u32>>,
    }

    impl FriProofSidecarSource for DummyFriSidecarSource {
        fn get_proof_oracle_stream(
            &mut self,
            _statement_versioned_hash: Bytes32,
        ) -> Option<Vec<u32>> {
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
    fn responder_prefixes_stream_length() {
        let mut responder = FriProofResponder {
            sidecar_source: DummyFriSidecarSource {
                response: Some(vec![7, 9, 13]),
            },
        };
        assert_eq!(run(&mut responder), vec![3, 7 | (9usize << 32), 13]);
    }

    #[test]
    fn missing_sidecar_returns_empty_response() {
        // A missing sidecar must return zero oracle words so the host
        // path triggers FriProofSidecarMissing, not a silent empty proof.
        let mut responder = FriProofResponder {
            sidecar_source: DummyFriSidecarSource { response: None },
        };
        assert_eq!(run(&mut responder), Vec::<usize>::new());
    }
}

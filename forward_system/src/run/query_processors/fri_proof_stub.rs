use super::*;
use crate::run::FriProofSidecarSource;
use std::sync::Arc;
use zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;

#[derive(Debug, Clone, Default)]
pub struct FriVerifierArtifacts;

/// Handles `FRI_PROOF_QUERY_ID` oracle queries.
#[derive(Debug, Clone)]
pub struct FriProofResponder<S: FriProofSidecarSource> {
    pub sidecar_source: S,
    pub artifacts: Option<Arc<FriVerifierArtifacts>>,
}

impl<S: FriProofSidecarSource> OracleQueryProcessor for FriProofResponder<S> {
    fn supported_query_ids(&self) -> Vec<u32> {
        Vec::new()
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        let _ = query_id;
        false
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        let _ = (&self.sidecar_source, &self.artifacts, query_id, query);
        DynUsizeIterator::from_constructor(Vec::new(), |response| response.iter().copied())
    }
}

use super::*;
use zk_ee::common_structs::da_commitment_scheme::DACommitmentScheme;
use zk_ee::oracle::query_ids::DA_COMMITMENT_SCHEME_QUERY_ID;

/// This processor handles DA commitment scheme request.
///
/// The data is consumed once per query and must be set initially.
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct DACommitmentSchemeResponder {
    pub da_commitment_scheme: Option<DACommitmentScheme>,
}

impl DACommitmentSchemeResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[DA_COMMITMENT_SCHEME_QUERY_ID];
}

impl OracleQueryProcessor for DACommitmentSchemeResponder {
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn process_memory_query(
        &mut self,
        query_id: u32,
        _input_word: usize,
        _memory: &dyn QuerierMemory,
        mode: RunMode,
        native_run_responses: &mut Vec<u32>,
        guest_run_responses: &mut Vec<u32>,
    ) {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));
        let data = self
            .da_commitment_scheme
            .take()
            .expect("io implementer data is none (second read or not set initially)");
        respond_to_every_target(
            mode,
            memory_response(&data),
            native_run_responses,
            guest_run_responses,
        );
    }
}

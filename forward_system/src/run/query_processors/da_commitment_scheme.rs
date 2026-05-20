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
    fn supported_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        Self::SUPPORTED_QUERY_IDS.contains(&query_id)
    }

    fn process(
        &mut self,
        query_id: u32,
        _input: &[u32],
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Result<Vec<u32>, InternalError> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        let data = self
            .da_commitment_scheme
            .take()
            .expect("io implementer data is none (second read or not set initially)");

        let mut result = Vec::new();
        (data as u8).write_words(&mut |w| result.push(w));
        Ok(result)
    }
}

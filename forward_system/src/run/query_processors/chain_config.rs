use super::*;
use zk_ee::oracle::query_ids::CHAIN_CONFIG_QUERY_ID;
use zk_ee::system::metadata::chain_config::ChainConfig;

#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug)]
pub struct ChainConfigResponder {
    pub chain_config: ChainConfig,
}

impl ChainConfigResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[CHAIN_CONFIG_QUERY_ID];
}

impl OracleQueryProcessor for ChainConfigResponder {
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
        respond_to_every_target(
            mode,
            memory_response(&self.chain_config),
            native_run_responses,
            guest_run_responses,
        );
    }
}

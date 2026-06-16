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
    fn supported_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        Self::SUPPORTED_QUERY_IDS.contains(&query_id)
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        _query: Vec<usize>,
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        DynUsizeIterator::from_constructor(self.chain_config, UsizeSerializable::iter)
    }
}

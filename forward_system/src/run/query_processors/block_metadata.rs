use super::*;
use oracle_provider::OracleQueryProcessor;
use zk_ee::oracle::query_ids::BLOCK_METADATA_QUERY_ID;
use zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BlockMetadataResponder {
    pub block_metadata: BlockMetadataFromOracle,
}

impl BlockMetadataResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[BLOCK_METADATA_QUERY_ID];
}

impl<M: MemorySource> OracleQueryProcessor<M> for BlockMetadataResponder {
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
        _memory: &M,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        DynUsizeIterator::from_constructor(self.block_metadata, UsizeSerializable::iter)
    }
}

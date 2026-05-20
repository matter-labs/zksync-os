use super::*;
use oracle_provider::OracleQueryProcessor;
use zk_ee::oracle::query_ids::BLOCK_METADATA_QUERY_ID;
use zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle;

#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug)]
pub struct BlockMetadataResponder {
    pub block_metadata: BlockMetadataFromOracle,
}

impl BlockMetadataResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[BLOCK_METADATA_QUERY_ID];
}

impl OracleQueryProcessor for BlockMetadataResponder {
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

        let mut result = Vec::new();
        self.block_metadata.write_words(&mut |w| result.push(w));
        Ok(result)
    }
}

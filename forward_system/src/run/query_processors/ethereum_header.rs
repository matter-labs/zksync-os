use super::*;
use alloy::consensus::Header;
use basic_bootloader::bootloader::block_flow::ethereum::oracle_queries::ETHEREUM_TARGET_HEADER_BUFFER_DATA_QUERY_ID;
use basic_bootloader::bootloader::block_flow::ethereum::oracle_queries::ETHEREUM_TARGET_HEADER_BUFFER_LEN_QUERY_ID;

use oracle_provider::OracleQueryProcessor;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EthereumTargetBlockHeaderResponder {
    pub target_header: Header,
    pub target_header_encoding: Vec<u8>,
}

impl EthereumTargetBlockHeaderResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[
        ETHEREUM_TARGET_HEADER_BUFFER_LEN_QUERY_ID,
        ETHEREUM_TARGET_HEADER_BUFFER_DATA_QUERY_ID,
    ];
}

impl OracleQueryProcessor for EthereumTargetBlockHeaderResponder {
    fn supported_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        Self::SUPPORTED_QUERY_IDS.contains(&query_id)
    }

    fn process(
        &mut self,
        query_id: u32,
        _input: &[u8],
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Result<Vec<u8>, InternalError> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        match query_id {
            ETHEREUM_TARGET_HEADER_BUFFER_LEN_QUERY_ID => {
                let len = self.target_header_encoding.len() as u32;
                AirbenderCodecV0::encode(&len)
                    .map_err(|_| internal_error!("encode header len failed"))
            }
            ETHEREUM_TARGET_HEADER_BUFFER_DATA_QUERY_ID => {
                AirbenderCodecV0::encode(&self.target_header_encoding)
                    .map_err(|_| internal_error!("encode header data failed"))
            }
            _ => {
                unreachable!()
            }
        }
    }
}

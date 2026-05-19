use super::*;
use alloy::consensus::Header;
use basic_bootloader::bootloader::block_flow::ethereum::oracle_queries::{
    ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID,
    ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID, ETHEREUM_WITHDRAWALS_BUFFER_DATA_QUERY_ID,
    ETHEREUM_WITHDRAWALS_BUFFER_LEN_QUERY_ID,
};
use crypto::MiniDigest;
use oracle_provider::OracleQueryProcessor;
use zk_ee::{oracle::query_ids::HISTORICAL_BLOCK_HASH_QUERY_ID, utils::Bytes32};

#[derive(Clone, Debug)]
pub struct EthereumCLResponder {
    pub withdrawals_list: Vec<u8>,
    pub parent_headers_list: Vec<Header>,
    pub parent_headers_encodings_list: Vec<Vec<u8>>,
}

impl EthereumCLResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[
        ETHEREUM_WITHDRAWALS_BUFFER_LEN_QUERY_ID,
        ETHEREUM_WITHDRAWALS_BUFFER_DATA_QUERY_ID,
        ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID,
        ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID,
        HISTORICAL_BLOCK_HASH_QUERY_ID,
    ];
}

impl OracleQueryProcessor for EthereumCLResponder {
    fn supported_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        Self::SUPPORTED_QUERY_IDS.contains(&query_id)
    }

    fn process(
        &mut self,
        query_id: u32,
        input: &[u8],
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Result<Vec<u8>, InternalError> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        match query_id {
            ETHEREUM_WITHDRAWALS_BUFFER_LEN_QUERY_ID => {
                let len = self.withdrawals_list.len() as u32;
                wincode::serialize(&len)
                    .map_err(|_| internal_error!("encode withdrawals len failed"))
            }
            ETHEREUM_WITHDRAWALS_BUFFER_DATA_QUERY_ID => wincode::serialize(&self.withdrawals_list)
                .map_err(|_| internal_error!("encode withdrawals data failed")),
            ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID => {
                let depth: u32 = wincode::deserialize(input)
                    .map_err(|_| internal_error!("decode historical depth failed"))?;
                assert!(depth < 256);
                let len = self.parent_headers_encodings_list[depth as usize].len() as u32;
                wincode::serialize(&len)
                    .map_err(|_| internal_error!("encode historical header len failed"))
            }
            ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID => {
                let depth: u32 = wincode::deserialize(input)
                    .map_err(|_| internal_error!("decode historical depth failed"))?;
                assert!(depth < 256);
                wincode::serialize(&self.parent_headers_encodings_list[depth as usize])
                    .map_err(|_| internal_error!("encode historical header data failed"))
            }
            HISTORICAL_BLOCK_HASH_QUERY_ID => {
                let depth: u32 = wincode::deserialize(input)
                    .map_err(|_| internal_error!("decode historical depth failed"))?;
                assert!(depth < 256);
                let hash: Bytes32 = self
                    .parent_headers_encodings_list
                    .get(depth as usize)
                    .map(|el| crypto::sha3::Keccak256::digest(el).into())
                    .unwrap_or(Bytes32::ZERO);
                wincode::serialize(&hash).map_err(|_| internal_error!("encode block hash failed"))
            }
            _ => {
                unreachable!()
            }
        }
    }
}

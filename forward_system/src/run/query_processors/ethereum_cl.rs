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
        input: &[u32],
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Result<Vec<u32>, InternalError> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        let mut result = Vec::new();
        match query_id {
            ETHEREUM_WITHDRAWALS_BUFFER_LEN_QUERY_ID => {
                (self.withdrawals_list.len() as u32).write_words(&mut |w| result.push(w));
            }
            ETHEREUM_WITHDRAWALS_BUFFER_DATA_QUERY_ID => {
                self.withdrawals_list.write_words(&mut |w| result.push(w));
            }
            ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID => {
                let depth: u32 = decode_input(input);
                assert!(depth < 256);
                (self.parent_headers_encodings_list[depth as usize].len() as u32)
                    .write_words(&mut |w| result.push(w));
            }
            ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID => {
                let depth: u32 = decode_input(input);
                assert!(depth < 256);
                self.parent_headers_encodings_list[depth as usize]
                    .write_words(&mut |w| result.push(w));
            }
            HISTORICAL_BLOCK_HASH_QUERY_ID => {
                let depth: u32 = decode_input(input);
                assert!(depth < 256);
                let hash: Bytes32 = self
                    .parent_headers_encodings_list
                    .get(depth as usize)
                    .map(|el| crypto::sha3::Keccak256::digest(el).into())
                    .unwrap_or(Bytes32::ZERO);
                hash.write_words(&mut |w| result.push(w));
            }
            _ => {
                unreachable!()
            }
        }
        Ok(result)
    }
}

/// Decode WordLayout-encoded u32 words back into a typed value.
fn decode_input<T: WordLayout>(input: &[u32]) -> T {
    let mut cursor = 0;
    T::read_words(&mut || {
        let w = input.get(cursor).copied().unwrap_or(0);
        cursor += 1;
        w
    })
}

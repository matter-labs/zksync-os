use super::*;
use alloy::consensus::Header;
use basic_bootloader::bootloader::block_flow::ethereum::oracle_queries::ETHEREUM_TARGET_HEADER_BUFFER_DATA_QUERY_ID;
use basic_bootloader::bootloader::block_flow::ethereum::oracle_queries::ETHEREUM_TARGET_HEADER_BUFFER_LEN_QUERY_ID;

use oracle_provider::OracleQueryProcessor;
use zk_ee::oracle::memory_io::host::write_dynamic_bytes;

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
        let response = match query_id {
            ETHEREUM_TARGET_HEADER_BUFFER_LEN_QUERY_ID => {
                memory_response(&(self.target_header_encoding.len() as u32))
            }
            ETHEREUM_TARGET_HEADER_BUFFER_DATA_QUERY_ID => {
                let mut response = Vec::new();
                write_dynamic_bytes(&self.target_header_encoding, &mut response);
                response
            }
            _ => unreachable!("not a target header query: 0x{query_id:08x}"),
        };
        respond_to_every_target(mode, response, native_run_responses, guest_run_responses);
    }
}

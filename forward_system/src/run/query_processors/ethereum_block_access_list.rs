use super::*;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::oracle_queries::{
    ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_DATA_QUERY_ID, ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_LEN_QUERY_ID,
};
use oracle_provider::OracleQueryProcessor;
use zk_ee::oracle::memory_io::host::write_dynamic_bytes;

/// Serves the RLP encoding of the block access list (EIP-7928) of the target block.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EthereumBlockAccessListResponder {
    pub access_list_encoding: Vec<u8>,
}

impl EthereumBlockAccessListResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[
        ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_LEN_QUERY_ID,
        ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_DATA_QUERY_ID,
    ];
}

impl OracleQueryProcessor for EthereumBlockAccessListResponder {
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
            ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_LEN_QUERY_ID => {
                memory_response(&(self.access_list_encoding.len() as u32))
            }
            ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_DATA_QUERY_ID => {
                let mut response = Vec::new();
                write_dynamic_bytes(&self.access_list_encoding, &mut response);
                response
            }
            _ => unreachable!("not a block access list query: 0x{query_id:08x}"),
        };
        respond_to_every_target(mode, response, native_run_responses, guest_run_responses);
    }
}

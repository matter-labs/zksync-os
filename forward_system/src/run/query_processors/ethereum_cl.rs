use super::*;
use alloy::consensus::Header;
use basic_bootloader::bootloader::block_flow::ethereum::oracle_queries::{
    ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID,
    ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID, ETHEREUM_WITHDRAWALS_BUFFER_DATA_QUERY_ID,
    ETHEREUM_WITHDRAWALS_BUFFER_LEN_QUERY_ID,
};
use crypto::MiniDigest;
use oracle_provider::OracleQueryProcessor;
use zk_ee::{
    oracle::{memory_io::host::write_dynamic_bytes, query_ids::HISTORICAL_BLOCK_HASH_QUERY_ID},
    utils::Bytes32,
};

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

impl EthereumCLResponder {
    /// The encoding of the header at `depth` (0 for the parent), as the input word of a historical
    /// header query sends it
    fn parent_header_encoding(&self, memory: &dyn QuerierMemory, input_word: usize) -> &[u8] {
        let depth = u32::read_input(memory, input_word).expect("must get historical depth");
        assert!(depth < 256);
        &self.parent_headers_encodings_list[depth as usize]
    }
}

impl OracleQueryProcessor for EthereumCLResponder {
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn process_memory_query(
        &mut self,
        query_id: u32,
        input_word: usize,
        memory: &dyn QuerierMemory,
        mode: RunMode,
        native_run_responses: &mut Vec<u32>,
        guest_run_responses: &mut Vec<u32>,
    ) {
        let mut response = Vec::new();
        match query_id {
            ETHEREUM_WITHDRAWALS_BUFFER_LEN_QUERY_ID => {
                (self.withdrawals_list.len() as u32).write_output(&mut response)
            }
            ETHEREUM_WITHDRAWALS_BUFFER_DATA_QUERY_ID => {
                write_dynamic_bytes(&self.withdrawals_list, &mut response)
            }
            ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID => {
                (self.parent_header_encoding(memory, input_word).len() as u32)
                    .write_output(&mut response)
            }
            ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID => write_dynamic_bytes(
                self.parent_header_encoding(memory, input_word),
                &mut response,
            ),
            HISTORICAL_BLOCK_HASH_QUERY_ID => {
                // the hashes of all 256 previous blocks, by depth, zero where unknown
                let hashes: [Bytes32; 256] = core::array::from_fn(|depth| {
                    self.parent_headers_encodings_list
                        .get(depth)
                        .map(|el| crypto::sha3::Keccak256::digest(el).into())
                        .unwrap_or(Bytes32::ZERO)
                });
                hashes.write_output(&mut response)
            }
            _ => unreachable!("not a CL query: 0x{query_id:08x}"),
        }
        respond_to_every_target(mode, response, native_run_responses, guest_run_responses);
    }
}

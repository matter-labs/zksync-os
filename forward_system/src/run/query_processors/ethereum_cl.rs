use super::*;
use alloy::consensus::Header;
use basic_bootloader::bootloader::block_flow::ethereum_block_flow::oracle_queries::{
    ETHEREUM_BLOB_POINT_QUERY_ID, ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID,
    ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID, ETHEREUM_WITHDRAWALS_BUFFER_DATA_QUERY_ID,
    ETHEREUM_WITHDRAWALS_BUFFER_LEN_QUERY_ID,
};
use crypto::MiniDigest;
use oracle_provider::OracleQueryProcessor;
use zk_ee::{oracle::ReadIterWrapper, system_io_oracle::HISTORICAL_BLOCK_HASH_QUERY_ID};

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
        ETHEREUM_BLOB_POINT_QUERY_ID,
    ];
}

impl<M: MemorySource> OracleQueryProcessor<M> for EthereumCLResponder {
    fn supported_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        Self::SUPPORTED_QUERY_IDS.contains(&query_id)
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        _memory: &M,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        match query_id {
            ETHEREUM_WITHDRAWALS_BUFFER_LEN_QUERY_ID => DynUsizeIterator::from_constructor(
                self.withdrawals_list.len() as u32,
                UsizeSerializable::iter,
            ),
            ETHEREUM_WITHDRAWALS_BUFFER_DATA_QUERY_ID => {
                DynUsizeIterator::from_constructor(self.withdrawals_list.clone(), |inner_ref| {
                    ReadIterWrapper::from(inner_ref.iter().copied())
                })
            }
            ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID => {
                let input: u32 =
                    u32::from_iter(&mut query.into_iter()).expect("must get historical depth");
                assert!(input < 256);
                DynUsizeIterator::from_constructor(
                    self.parent_headers_encodings_list[input as usize].len() as u32,
                    UsizeSerializable::iter,
                )
            }
            ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID => {
                let input: u32 =
                    u32::from_iter(&mut query.into_iter()).expect("must get historical depth");
                assert!(input < 256);
                DynUsizeIterator::from_constructor(
                    self.parent_headers_encodings_list[input as usize].clone(),
                    |inner_ref| ReadIterWrapper::from(inner_ref.iter().copied()),
                )
            }
            HISTORICAL_BLOCK_HASH_QUERY_ID => {
                let input: u32 =
                    u32::from_iter(&mut query.into_iter()).expect("must get historical depth");
                assert!(input < 256);
                let hash: Bytes32 = self
                    .parent_headers_encodings_list
                    .get(input as usize)
                    .map(|el| crypto::sha3::Keccak256::digest(el).into())
                    .unwrap_or(Bytes32::ZERO);
                DynUsizeIterator::from_constructor(hash, UsizeSerializable::iter)
            }
            _ => {
                unreachable!()
            }
        }
    }
}

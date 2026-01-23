use ruint::aliases::{B160, U256};
use zk_ee::system::metadata::basic_metadata::BasicTransactionMetadata;
use zk_ee::{types_config::EthereumIOTypesConfig, utils::Bytes32};

#[derive(Clone, Debug)]
pub struct EthereumTransactionMetadata<const MAX_BLOBS: usize> {
    pub tx_origin: B160,
    pub tx_gas_price: U256,
    pub blobs: arrayvec::ArrayVec<Bytes32, MAX_BLOBS>,
}

impl<const MAX_BLOBS: usize> EthereumTransactionMetadata<MAX_BLOBS> {
    pub fn empty() -> Self {
        Self {
            tx_origin: B160::ZERO,
            tx_gas_price: U256::ZERO,
            blobs: arrayvec::ArrayVec::new(),
        }
    }
}

impl<const MAX_BLOBS: usize> BasicTransactionMetadata<EthereumIOTypesConfig>
    for EthereumTransactionMetadata<MAX_BLOBS>
{
    fn tx_origin(&self) -> B160 {
        self.tx_origin
    }
    fn tx_gas_price(&self) -> U256 {
        self.tx_gas_price
    }
    fn num_blobs(&self) -> usize {
        self.blobs.len()
    }
    fn get_blob_hash(&self, idx: usize) -> Option<Bytes32> {
        self.blobs.get(idx).copied()
    }
}

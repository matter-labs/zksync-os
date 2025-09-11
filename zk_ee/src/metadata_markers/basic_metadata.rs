use crate::{types_config::SystemIOTypesConfig, utils::Bytes32};
use ruint::aliases::U256;

// NOTE: such may or may be NOT a part of the protocol. E.g. some concrete implementation may
// hardcode limits, while others - keep them dynamic
pub trait BasicBlockMetadata<IOTypes: SystemIOTypesConfig> {
    fn chain_id(&self) -> u64;
    fn block_number(&self) -> u64;
    fn block_historical_hash(&self, depth: u64) -> Option<Bytes32>;
    fn block_timestamp(&self) -> u64;
    fn block_randomness(&self) -> Option<Bytes32>;
    fn coinbase(&self) -> IOTypes::Address;
    fn block_gas_limit(&self) -> u64 {
        u64::MAX
    }
    fn individual_tx_gas_limit(&self) -> u64 {
        u64::MAX
    }
    fn eip1559_basefee(&self) -> U256 {
        U256::ZERO
    }
    fn max_blobs(&self) -> usize {
        0
    }
    fn blobs_gas_limit(&self) -> u64 {
        u64::MAX
    }
    fn blob_base_fee_per_gas(&self) -> U256 {
        U256::ZERO
    }
}

pub trait BasicTransactionMetadata<IOTypes: SystemIOTypesConfig> {
    fn tx_origin(&self) -> IOTypes::Address;
    fn tx_gas_price(&self) -> U256;
    // fn tx_gas_limit(&self) -> u64;
    fn num_blobs(&self) -> usize {
        0
    }
    fn get_blob_hash(&self, _idx: usize) -> Option<Bytes32> {
        None
    }
}

pub trait ZkSpecificPricingMetadata {
    fn gas_per_pubdata(&self) -> U256 {
        U256::ZERO
    }
    fn native_price(&self) -> U256 {
        U256::ZERO
    }
    fn get_pubdata_limit(&self) -> u64 {
        0
    }
}

pub trait BasicMetadata<IOTypes: SystemIOTypesConfig>:
    BasicBlockMetadata<IOTypes> + BasicTransactionMetadata<IOTypes>
{
    type TransactionMetadata;
    fn set_transaction_metadata(&mut self, tx_level_metadata: Self::TransactionMetadata);
}

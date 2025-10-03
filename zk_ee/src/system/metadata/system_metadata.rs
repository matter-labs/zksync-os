use super::basic_metadata::*;
use crate::types_config::SystemIOTypesConfig;
use crate::utils::Bytes32;
use ruint::aliases::U256;

/// Aggregates block-level, tx-level, and optional dynamic metadata into one value.
pub struct SystemMetadata<
    IOTypes: SystemIOTypesConfig,
    B: BasicBlockMetadata<IOTypes>,
    TX: BasicTransactionMetadata<IOTypes>,
> {
    /// Provider of block-scoped metadata.
    pub block_level: B,

    /// Provider of metadata for the current transaction.
    pub tx_level: TX,

    pub _marker: core::marker::PhantomData<IOTypes>,
}

/// Forwards to `block_level`.
impl<
        IOTypes: SystemIOTypesConfig,
        B: BasicBlockMetadata<IOTypes>,
        TX: BasicTransactionMetadata<IOTypes>,
    > BasicBlockMetadata<IOTypes> for SystemMetadata<IOTypes, B, TX>
{
    fn chain_id(&self) -> u64 {
        self.block_level.chain_id()
    }
    fn block_number(&self) -> u64 {
        self.block_level.block_number()
    }
    fn block_historical_hash(&self, depth: u64) -> Option<Bytes32> {
        self.block_level.block_historical_hash(depth)
    }
    fn block_timestamp(&self) -> u64 {
        self.block_level.block_timestamp()
    }
    fn block_randomness(&self) -> Option<Bytes32> {
        self.block_level.block_randomness()
    }
    fn coinbase(&self) -> IOTypes::Address {
        self.block_level.coinbase()
    }
    fn block_gas_limit(&self) -> u64 {
        self.block_level.block_gas_limit()
    }
    fn individual_tx_gas_limit(&self) -> u64 {
        self.block_level.individual_tx_gas_limit()
    }
    fn eip1559_basefee(&self) -> U256 {
        self.block_level.eip1559_basefee()
    }
    fn max_blobs(&self) -> usize {
        self.block_level.max_blobs()
    }
    fn blobs_gas_limit(&self) -> u64 {
        self.block_level.blobs_gas_limit()
    }
    fn blob_base_fee_per_gas(&self) -> U256 {
        self.block_level.blob_base_fee_per_gas()
    }
}

/// Forwards to `tx_level`.
impl<
        IOTypes: SystemIOTypesConfig,
        B: BasicBlockMetadata<IOTypes>,
        TX: BasicTransactionMetadata<IOTypes>,
    > BasicTransactionMetadata<IOTypes> for SystemMetadata<IOTypes, B, TX>
{
    fn tx_origin(&self) -> IOTypes::Address {
        self.tx_level.tx_origin()
    }
    fn tx_gas_price(&self) -> U256 {
        self.tx_level.tx_gas_price()
    }
    fn num_blobs(&self) -> usize {
        self.tx_level.num_blobs()
    }
    fn get_blob_hash(&self, idx: usize) -> Option<Bytes32> {
        self.tx_level.get_blob_hash(idx)
    }
}

/// Assumes that ZK specific pricing metadata is implemented at the block level.
impl<
        IOTypes: SystemIOTypesConfig,
        B: BasicBlockMetadata<IOTypes> + ZkSpecificPricingMetadata,
        TX: BasicTransactionMetadata<IOTypes>,
    > ZkSpecificPricingMetadata for SystemMetadata<IOTypes, B, TX>
{
    fn gas_per_pubdata(&self) -> U256 {
        self.block_level.gas_per_pubdata()
    }
    fn native_price(&self) -> U256 {
        self.block_level.native_price()
    }
    fn get_pubdata_limit(&self) -> u64 {
        self.block_level.get_pubdata_limit()
    }
}

impl<
        IOTypes: SystemIOTypesConfig,
        B: BasicBlockMetadata<IOTypes>,
        TX: BasicTransactionMetadata<IOTypes>,
    > BasicMetadata<IOTypes> for SystemMetadata<IOTypes, B, TX>
{
    type TransactionMetadata = TX;

    fn set_transaction_metadata(&mut self, tx_level_metadata: Self::TransactionMetadata) {
        self.tx_level = tx_level_metadata;
    }
}

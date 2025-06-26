use crate::bootloader::block_header::BlockHeader;
use ruint::aliases::B160;
use zk_ee::system::{IOResultKeeper, NopResultKeeper};
use zk_ee::types_config::EthereumIOTypesConfig;
///
/// This module contains definition of the result keeper trait.
///
/// Result keeper structure that will be called during execution to save the block execution result.
/// It's needed for sequencing(to collect receipts, diffs, pubdata).
///
/// Since we will not use it during the proving, it will operate with rust types.
///
use zksync_os_error::core::tx_valid::ValidationError as InvalidTransaction;

pub struct TxProcessingOutput<'a> {
    pub status: bool,
    pub output: &'a [u8],
    pub contract_address: Option<B160>,
    pub gas_used: u64,
    pub gas_refunded: u64,
    #[cfg(feature = "report_native")]
    pub native_used: u64,
}

pub trait ResultKeeperExt: IOResultKeeper<EthereumIOTypesConfig> {
    fn tx_processed(&mut self, _tx_result: Result<TxProcessingOutput<'_>, InvalidTransaction>) {}

    fn block_sealed(&mut self, _block_header: BlockHeader) {}

    fn get_gas_used(&self) -> u64 {
        0u64
    }
}

impl ResultKeeperExt for NopResultKeeper {}

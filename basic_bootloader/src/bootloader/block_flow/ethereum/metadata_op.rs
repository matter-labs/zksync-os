use zk_ee::system::metadata::basic_metadata::BasicBlockMetadata;
use zk_ee::{
    internal_error,
    oracle::IOOracle,
    system::{
        errors::internal::InternalError, metadata::system_metadata::SystemMetadata, SystemTypes,
        MAX_BLOBS_PER_BLOCK, MAX_BLOCK_GAS_LIMIT,
    },
    types_config::EthereumIOTypesConfig,
};

use crate::bootloader::transaction_flow::ethereum::tx_level_metadata::EthereumTransactionMetadata;

use super::{
    block_header::HeaderAndHistory, BasicBootloaderExecutionConfig, EthereumMetadataOp,
    MetadataInitOp,
};

pub type EthereumBlockMetadata = SystemMetadata<
    EthereumIOTypesConfig,
    HeaderAndHistory,
    EthereumTransactionMetadata<{ MAX_BLOBS_PER_BLOCK }>,
>;

impl<S: SystemTypes<Metadata = EthereumBlockMetadata>> MetadataInitOp<S> for EthereumMetadataOp {
    fn metadata_op<'a, Config: BasicBootloaderExecutionConfig>(
        oracle: &mut impl IOOracle,
        allocator: S::Allocator,
    ) -> Result<<S as SystemTypes>::Metadata, InternalError> {
        // make header's buffer, parse, make into our internal structure, save hash
        let header = HeaderAndHistory::new(oracle, allocator.clone())?;

        // NOTE: we do NOT check the following:
        // - there is some historical header at all
        // - excess blob gas is one coming from the parent
        // - potentially EIP-1559 params

        if header.block_gas_limit() > MAX_BLOCK_GAS_LIMIT {
            return Err(internal_error!("block gas limit is too high"));
        }

        let metadata = EthereumBlockMetadata {
            block_level: header,
            tx_level: EthereumTransactionMetadata::empty(),
            _marker: core::marker::PhantomData,
        };

        Ok(metadata)
    }
}

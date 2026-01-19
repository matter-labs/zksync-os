use super::*;
use crate::bootloader::constants::MAX_BLOCK_GAS_LIMIT;
use zk_ee::internal_error;
use zk_ee::oracle::query_ids::BLOCK_METADATA_QUERY_ID;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::metadata::basic_metadata::BasicBlockMetadata;
use zk_ee::system::metadata::zk_metadata::{BlockMetadataFromOracle, TxLevelMetadata, ZkMetadata};
use zk_ee::system::{SystemTypes, MAX_TX_GAS_LIMIT};

impl<S: SystemTypes<Metadata = zk_ee::system::metadata::zk_metadata::ZkMetadata>> MetadataInitOp<S>
    for zk_ee::system::metadata::zk_metadata::ZkMetadata
{
    fn metadata_op<Config: BasicBootloaderExecutionConfig>(
        oracle: &mut impl IOOracle,
        _allocator: S::Allocator,
    ) -> Result<<S as SystemTypes>::Metadata, InternalError> {
        let block_level_metadata: BlockMetadataFromOracle =
            oracle.query_with_empty_input(BLOCK_METADATA_QUERY_ID)?;

        let metadata = ZkMetadata {
            tx_level: TxLevelMetadata::default(),
            block_level: block_level_metadata,
            _marker: core::marker::PhantomData,
        };

        if metadata.block_gas_limit() > MAX_BLOCK_GAS_LIMIT
            || metadata.individual_tx_gas_limit() > MAX_TX_GAS_LIMIT
        {
            return Err(internal_error!("block or tx gas limit is too high"));
        }

        Ok(metadata)
    }
}

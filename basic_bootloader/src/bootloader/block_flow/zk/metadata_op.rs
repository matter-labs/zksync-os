use super::*;
use zk_ee::internal_error;
use zk_ee::oracle::query_ids::BLOCK_METADATA_QUERY_ID;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::metadata::basic_metadata::BasicBlockMetadata;
use zk_ee::system::metadata::zk_metadata::{BlockMetadataFromOracle, TxLevelMetadata, ZkMetadata};
use zk_ee::system::{Resources, SystemTypes};

impl<S: SystemTypes<Metadata = zk_ee::system::metadata::zk_metadata::ZkMetadata>> MetadataInitOp<S>
    for zk_ee::system::metadata::zk_metadata::ZkMetadata
{
    fn metadata_op<Config: BasicBootloaderExecutionConfig>(
        oracle: &mut impl IOOracle,
        _allocator: S::Allocator,
        chain_config: zk_ee::system::metadata::chain_config::ChainConfig,
    ) -> Result<<S as SystemTypes>::Metadata, InternalError> {
        chain_config.validate()?;

        let block_level_metadata: BlockMetadataFromOracle =
            oracle.query_with_empty_input(BLOCK_METADATA_QUERY_ID)?;

        let metadata = ZkMetadata {
            tx_level: TxLevelMetadata::default(),
            block_level: block_level_metadata,
            chain_config,
            _marker: core::marker::PhantomData,
        };

        let individual_tx_gas_limit = core::cmp::min(
            metadata.block_gas_limit(),
            metadata.chain_config.max_tx_gas_limit(),
        );

        // Both limits must be representable as ergs of this system.
        let max_gas = <S::Resources as Resources>::MAX_LEGACY_GAS;
        if metadata.block_gas_limit() > max_gas || individual_tx_gas_limit > max_gas {
            return Err(internal_error!("block or tx gas limit is too high"));
        }

        Ok(metadata)
    }
}

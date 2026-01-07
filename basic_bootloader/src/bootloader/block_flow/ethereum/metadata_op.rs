use zk_ee::{
    system::{metadata::system_metadata::SystemMetadata, MAX_BLOBS_PER_BLOCK},
    types_config::EthereumIOTypesConfig,
};

use crate::bootloader::transaction_flow::ethereum::tx_level_metadata::EthereumTransactionMetadata;

use super::block_header::HeaderAndHistory;

pub type EthereumBlockMetadata = SystemMetadata<
    EthereumIOTypesConfig,
    HeaderAndHistory,
    EthereumTransactionMetadata<{ MAX_BLOBS_PER_BLOCK }>,
>;

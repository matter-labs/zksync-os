//! Block metadata of a transaction range: the Glamsterdam header, witnessed
//! and hashed, as the system's block-level metadata.

use super::super::block_header::{BlobSchedule, GlamsterdamHeader};
use crate::bootloader::block_flow::ethereum::block_hashes_cache::{
    BlockHashMetadataRequest, BlockHashesCache,
};
use crate::bootloader::block_flow::ethereum::oracle_queries::{
    ETHEREUM_TARGET_HEADER_BUFFER_DATA_QUERY_ID, ETHEREUM_TARGET_HEADER_BUFFER_LEN_QUERY_ID,
};
use crate::bootloader::block_flow::MetadataInitOp;
use crate::bootloader::config::BasicBootloaderExecutionConfig;
use crate::bootloader::transaction_flow::ethereum::tx_level_metadata::EthereumTransactionMetadata;
use crypto::MiniDigest;
use ruint::aliases::{B160, U256};
use zk_ee::internal_error;
use zk_ee::oracle::memory_io::get_bytes_with_length_query;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::metadata::basic_metadata::BasicBlockMetadata;
use zk_ee::system::metadata::chain_config::ChainConfig;
use zk_ee::system::metadata::dynamic_metadata_responder::DynamicMetadataResponder;
use zk_ee::system::metadata::system_metadata::SystemMetadata;
use zk_ee::system::{Resources, SystemTypes, GAS_PER_BLOB, MAX_BLOBS_PER_TX};
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;

/// The blob schedule the range runs under (Sepolia Glamsterdam: BPO2)
pub const BLOB_SCHEDULE: BlobSchedule = BlobSchedule::BPO2;

pub struct GlamsterdamHeaderAndHistory {
    pub chain_id: u64,
    pub header: GlamsterdamHeader,
    pub history_cache: core::cell::UnsafeCell<BlockHashesCache>,
    pub computed_header_hash: Bytes32,
    pub computed_blob_base_fee_per_gas: U256,
}

impl BasicBlockMetadata<EthereumIOTypesConfig> for GlamsterdamHeaderAndHistory {
    fn block_number(&self) -> u64 {
        self.header.base.number
    }
    fn block_historical_hash(&self, depth: u64) -> Option<Bytes32> {
        let depth = depth - 1;
        if depth < 256 {
            // SAFETY: the cache is only ever accessed through this method, one call at a time
            unsafe {
                Some(
                    self.history_cache
                        .as_mut_unchecked()
                        .get_metadata_with_bookkeeping::<BlockHashMetadataRequest>(depth as u8),
                )
            }
        } else {
            None
        }
    }
    fn block_timestamp(&self) -> u64 {
        self.header.base.timestamp
    }
    fn block_randomness(&self) -> Option<Bytes32> {
        Some(self.header.base.mix_hash)
    }
    fn coinbase(&self) -> B160 {
        self.header.base.beneficiary
    }
    fn block_gas_limit(&self) -> u64 {
        self.header.base.gas_limit
    }
    fn eip1559_basefee(&self) -> U256 {
        U256::from(self.header.base.base_fee_per_gas)
    }
    fn max_blobs(&self) -> usize {
        BLOB_SCHEDULE.max_blobs_per_block as usize
    }
    fn blobs_gas_limit(&self) -> u64 {
        BLOB_SCHEDULE.max_blob_gas_per_block()
    }
    fn blob_base_fee_per_gas(&self) -> U256 {
        self.computed_blob_base_fee_per_gas
    }
}

impl GlamsterdamHeaderAndHistory {
    /// The slot number of the block (EIP-7843)
    pub fn slot_number(&self) -> u64 {
        self.header.slot_number
    }

    pub fn new(
        oracle: &mut impl IOOracle,
        allocator: impl core::alloc::Allocator,
    ) -> Result<Self, InternalError> {
        let encoding = get_bytes_with_length_query::<_, (), _>(
            oracle,
            ETHEREUM_TARGET_HEADER_BUFFER_LEN_QUERY_ID,
            ETHEREUM_TARGET_HEADER_BUFFER_DATA_QUERY_ID,
            (),
            allocator,
        )?
        .ok_or(internal_error!("target header is empty"))?;
        let header = GlamsterdamHeader::parse(encoding.as_slice())
            .map_err(|_| internal_error!("must parse the target header"))?;
        let computed_header_hash = crypto::sha3::Keccak256::digest(encoding.as_slice()).into();
        let cache = BlockHashesCache::from_oracle(oracle)?;
        let computed_blob_base_fee_per_gas =
            BLOB_SCHEDULE.blob_base_fee(header.base.excess_blob_gas);
        Ok(Self {
            // Sepolia; the header carries no chain id
            chain_id: 11155111,
            header,
            history_cache: core::cell::UnsafeCell::new(cache),
            computed_header_hash,
            computed_blob_base_fee_per_gas,
        })
    }
}

pub type GlamsterdamBlockMetadata = SystemMetadata<
    EthereumIOTypesConfig,
    GlamsterdamHeaderAndHistory,
    EthereumTransactionMetadata<{ MAX_BLOBS_PER_TX }>,
    ChainConfig,
>;

pub struct GlamsterdamMetadataOp;

impl<S: SystemTypes<Metadata = GlamsterdamBlockMetadata>> MetadataInitOp<S>
    for GlamsterdamMetadataOp
{
    fn metadata_op<Config: BasicBootloaderExecutionConfig>(
        oracle: &mut impl IOOracle,
        allocator: S::Allocator,
        base_chain_config: ChainConfig,
    ) -> Result<S::Metadata, InternalError> {
        base_chain_config.validate()?;
        let header = GlamsterdamHeaderAndHistory::new(oracle, allocator)?;
        if header.block_gas_limit() > <S::Resources as Resources>::MAX_LEGACY_GAS {
            return Err(internal_error!("block gas limit is too high"));
        }
        let chain_config = ChainConfig::new(
            header.chain_id,
            base_chain_config.fri_proof_verification_enabled(),
            base_chain_config.max_tx_gas_limit(),
        )?;
        Ok(GlamsterdamBlockMetadata {
            block_level: header,
            tx_level: EthereumTransactionMetadata::empty(),
            chain_config,
            _marker: core::marker::PhantomData,
        })
    }
}

const _: () = assert!(GAS_PER_BLOB == 1 << 17);

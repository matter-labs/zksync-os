use crate::bootloader::block_flow::chain_check::ChainChecker;
use crate::bootloader::block_flow::ethereum::oracle_queries::ETHEREUM_TARGET_HEADER_BUFFER_DATA_QUERY_ID;
use crate::bootloader::block_flow::ethereum::oracle_queries::ETHEREUM_TARGET_HEADER_BUFFER_LEN_QUERY_ID;
use crate::bootloader::errors::BootloaderSubsystemError;
use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::RlpListDecode;
use crate::bootloader::transaction_flow::ethereum::LogsBloom;
use core::alloc::Allocator;
use crypto::MiniDigest;
use ruint::aliases::B160;
use ruint::aliases::U256;
use zk_ee::internal_error;
use zk_ee::system::metadata::dynamic_metadata_responder::DynamicMetadataResponder;

use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::metadata::basic_metadata::BasicBlockMetadata;
use zk_ee::system::GAS_PER_BLOB;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;

use super::block_hashes_cache::BlockHashMetadataRequest;
use super::block_hashes_cache::BlockHashesCache;
use super::utils::fake_exponential;
use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser;

pub const MIN_BASE_FEE_PER_BLOB_GAS: u64 = 1;
pub const BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE: u64 = 5007716;
pub const BLOB_BASE_FEE_UPDATE_FRACTION: u64 = BLOB_BASE_FEE_UPDATE_FRACTION_PRAGUE;

const MAX_BLOBS_PER_BLOCK: usize = 9;
const TARGET_BLOBS_PER_BLOCK: u64 = 6;
const TARGET_BLOB_GAS_PER_BLOCK: u64 = GAS_PER_BLOB * TARGET_BLOBS_PER_BLOCK;

const PECTRA_EL_FORK_BLOCK_NUMBER: u64 = 22431084;

const EIP_1559_BASE_FEE_MAX_CHANGE_DENOMINATOR: u64 = 8;
const EIP_1559_ELASTICITY_MULTIPLIER: u64 = 2;
const EIP_1559_MIN_GAS_LIMIT: u64 = 5000;

#[derive(Clone, Copy, Debug)]
pub struct PectraForkHeader {
    // Default fields
    pub parent_hash: Bytes32,
    pub ommers_hash: Bytes32,
    pub beneficiary: B160,
    pub state_root: Bytes32,
    pub transactions_root: Bytes32,
    pub receipts_root: Bytes32,
    pub logs_bloom: LogsBloom,
    pub difficulty: U256,
    pub number: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    // 32 bytes or less, but variable length
    pub extra_data: (Bytes32, usize),
    pub mix_hash: Bytes32,
    // fixed length
    pub nonce: [u8; 8],

    // EIP-1559
    pub base_fee_per_gas: u64,

    pub withdrawals_root: Bytes32,

    // EIP-4844
    pub blob_gas_used: u64,
    pub excess_blob_gas: u64,

    pub parent_beacon_block_root: Bytes32,
    pub requests_hash: Bytes32,
}

impl PectraForkHeader {
    pub fn from_relection(header: PectraForkHeaderReflection<'_>) -> Self {
        let PectraForkHeaderReflection {
            parent_hash,
            ommers_hash,
            beneficiary,
            state_root,
            transactions_root,
            receipts_root,
            logs_bloom,
            difficulty,
            number,
            gas_limit,
            gas_used,
            timestamp,
            extra_data,
            mix_hash,
            nonce,
            base_fee_per_gas,
            withdrawals_root,
            blob_gas_used,
            excess_blob_gas,
            parent_beacon_block_root,
            requests_hash,
        } = header;
        let extra_data_len = header.extra_data.len();
        let extra_data = {
            let mut buffer = Bytes32::zero();
            buffer.as_u8_array_mut()[..extra_data_len].copy_from_slice(extra_data);

            buffer
        };

        Self {
            parent_hash: Bytes32::from_array(*parent_hash),
            ommers_hash: Bytes32::from_array(*ommers_hash),
            beneficiary: B160::from_be_bytes(*beneficiary),
            state_root: Bytes32::from_array(*state_root),
            transactions_root: Bytes32::from_array(*transactions_root),
            receipts_root: Bytes32::from_array(*receipts_root),
            logs_bloom: LogsBloom::from_bytes(logs_bloom),
            difficulty: U256::from_be_slice(difficulty),
            number,
            gas_limit,
            gas_used,
            timestamp,
            extra_data: (extra_data, extra_data_len),
            mix_hash: Bytes32::from_array(*mix_hash),
            nonce: *nonce,
            base_fee_per_gas,
            withdrawals_root: Bytes32::from_array(*withdrawals_root),
            blob_gas_used,
            excess_blob_gas,
            parent_beacon_block_root: Bytes32::from_array(*parent_beacon_block_root),
            requests_hash: Bytes32::from_array(*requests_hash),
        }
    }
}

pub struct HeaderAndHistory {
    pub chain_id: u64,
    pub header: PectraForkHeader,
    pub history_cache: core::cell::UnsafeCell<BlockHashesCache>,
    pub computed_header_hash: Bytes32,
    pub computed_blob_base_fee_per_gas: U256,
}

impl BasicBlockMetadata<EthereumIOTypesConfig> for HeaderAndHistory {
    fn chain_id(&self) -> u64 {
        self.chain_id
    }
    fn block_number(&self) -> u64 {
        self.header.number
    }
    fn block_historical_hash(&self, depth: u64) -> Option<Bytes32> {
        if depth < 256 {
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
        self.header.timestamp
    }
    fn block_randomness(&self) -> Option<Bytes32> {
        Some(self.header.mix_hash)
    }
    fn coinbase(&self) -> B160 {
        self.header.beneficiary
    }
    fn block_gas_limit(&self) -> u64 {
        self.header.gas_limit
    }
    fn individual_tx_gas_limit(&self) -> u64 {
        self.block_gas_limit()
    }
    fn eip1559_basefee(&self) -> U256 {
        U256::from(self.header.base_fee_per_gas)
    }
    fn max_blobs(&self) -> usize {
        MAX_BLOBS_PER_BLOCK
    }
    fn blobs_gas_limit(&self) -> u64 {
        self.max_blobs() as u64 * GAS_PER_BLOB
    }
    fn blob_base_fee_per_gas(&self) -> U256 {
        self.computed_blob_base_fee_per_gas
    }
}

impl HeaderAndHistory {
    pub fn new(
        oracle: &mut impl IOOracle,
        allocator: impl core::alloc::Allocator,
    ) -> Result<Self, InternalError> {
        let chain_id = 1u64;
        // get buffer
        let target_header_buffer = oracle.get_bytes_from_query(
            ETHEREUM_TARGET_HEADER_BUFFER_LEN_QUERY_ID,
            ETHEREUM_TARGET_HEADER_BUFFER_DATA_QUERY_ID,
            &(),
            allocator,
        )?;
        let target_header_buffer = target_header_buffer.expect("target header is not empty slice");
        let target_header =
            PectraForkHeaderReflection::decode_list_full(target_header_buffer.as_slice())
                .map_err(|_| internal_error!("must parse target header from bytes"))?;
        let computed_header_hash =
            crypto::sha3::Keccak256::digest(target_header_buffer.as_slice()).into();
        let header = PectraForkHeader::from_relection(target_header);
        let cache = BlockHashesCache::from_oracle(oracle)?;

        // TODO: consider if it's numerically stable over u64
        let computed_blob_base_fee_per_gas = fake_exponential(
            U256::from(MIN_BASE_FEE_PER_BLOB_GAS),
            &U256::from(header.excess_blob_gas),
            &U256::from(BLOB_BASE_FEE_UPDATE_FRACTION),
        );

        Ok(Self {
            chain_id,
            header,
            history_cache: core::cell::UnsafeCell::new(cache),
            computed_header_hash,
            computed_blob_base_fee_per_gas,
        })
    }
}

// and we need a simple reflection to parse block hashes chain
#[derive(Clone, Copy, Debug)]
pub struct PectraForkHeaderReflection<'a> {
    // Default fields
    pub parent_hash: &'a [u8; 32],
    pub ommers_hash: &'a [u8; 32],
    pub beneficiary: &'a [u8; 20],
    pub state_root: &'a [u8; 32],
    pub transactions_root: &'a [u8; 32],
    pub receipts_root: &'a [u8; 32],
    pub logs_bloom: &'a [u8; 256],
    pub difficulty: &'a [u8],
    pub number: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    // 32 bytes or less, but variable length
    pub extra_data: &'a [u8],
    pub mix_hash: &'a [u8; 32],
    // fixed length
    pub nonce: &'a [u8; 8],

    // EIP-1559
    pub base_fee_per_gas: u64,

    pub withdrawals_root: &'a [u8; 32],

    // EIP-4844
    pub blob_gas_used: u64,
    pub excess_blob_gas: u64,

    pub parent_beacon_block_root: &'a [u8; 32],
    pub requests_hash: &'a [u8; 32],
}

// TODO: generalize RLP parser to return custom error, to be used
// both by txs and here.
impl<'a> RlpListDecode<'a> for PectraForkHeaderReflection<'a> {
    fn decode_list_body(
        r: &mut minimal_rlp_parser::Rlp<'a>,
    ) -> Result<Self, crate::bootloader::errors::InvalidTransaction> {
        let parent_hash = r.bytes_exact()?;
        let ommers_hash = r.bytes_exact()?;
        let beneficiary = r.bytes_exact()?;
        let state_root = r.bytes_exact()?;
        let transactions_root = r.bytes_exact()?;
        let receipts_root = r.bytes_exact()?;
        let logs_bloom = r.bytes_exact()?;
        let difficulty = r.bytes()?;
        let number = r.u64()?;
        let gas_limit = r.u64()?;
        let gas_used = r.u64()?;
        let timestamp = r.u64()?;
        // 32 bytes or less, but variable length
        let extra_data: &'a [u8] = r.bytes()?;
        if extra_data.len() > 32 {
            return Err(crate::bootloader::errors::InvalidTransaction::InvalidStructure);
        }
        let mix_hash = r.bytes_exact()?;
        // fixed length
        let nonce = r.bytes_exact()?;
        let base_fee_per_gas = r.u64()?;

        let withdrawals_root = r.bytes_exact()?;

        let blob_gas_used = r.u64()?;
        let excess_blob_gas = r.u64()?;

        let parent_beacon_block_root = r.bytes_exact()?;
        let requests_hash = r.bytes_exact()?;

        let new = Self {
            parent_hash,
            ommers_hash,
            beneficiary,
            state_root,
            transactions_root,
            receipts_root,
            logs_bloom,
            difficulty,
            number,
            gas_limit,
            gas_used,
            timestamp,
            extra_data,
            mix_hash,
            nonce,
            base_fee_per_gas,
            withdrawals_root,
            blob_gas_used,
            excess_blob_gas,
            parent_beacon_block_root,
            requests_hash,
        };

        Ok(new)
    }
}

impl ChainChecker for PectraForkHeader {
    type ExtraData = BlockHashesCache;
    type Output = Bytes32;

    fn verify_chain<A: Allocator + Clone>(
        &self,
        current_block_number: u64,
        verification_depth: usize,
        oracle: &mut impl IOOracle,
        extra_data: &Self::ExtraData,
        allocator: A,
    ) -> Result<Self::Output, BootloaderSubsystemError> {
        use crate::bootloader::block_flow::ethereum::oracle_queries::ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID;
        use crate::bootloader::block_flow::ethereum::oracle_queries::ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID;

        let history_cache = extra_data;
        assert!(verification_depth > 0);
        assert_eq!(self.number, current_block_number);
        assert!(current_block_number >= verification_depth as u64); // Do not want underflows anywhere beloe

        let mut block_headers_hasher = <crypto::sha3::Keccak256 as crypto::MiniDigest>::new();
        let mut initial_state_commitment = Bytes32::ZERO;
        let mut parent_to_expect = self.parent_hash;

        for depth in 0..verification_depth {
            let block_number = current_block_number - 1 - (depth as u64);
            // we do not expect to have any practical implementation that go across header formats,
            // so we assert here
            assert!(block_number >= PECTRA_EL_FORK_BLOCK_NUMBER);

            let buffer = oracle
                .get_bytes_from_query(
                    ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID,
                    ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID,
                    &(depth as u32),
                    allocator.clone(),
                )
                .expect("must get buffer for historical header")
                .expect("buffer for historical header is not empty");
            let historical_header = PectraForkHeaderReflection::decode_list_full(buffer.as_slice())
                .expect("must parse historical header");
            crypto::MiniDigest::update(&mut block_headers_hasher, buffer.as_slice());
            let computed_header_hash: Bytes32 =
                crypto::MiniDigest::finalize_reset(&mut block_headers_hasher).into();
            assert_eq!(history_cache.cache_entry(depth), &computed_header_hash,);
            assert_eq!(historical_header.number, block_number);
            assert_eq!(
                &parent_to_expect,
                &computed_header_hash,
                "parent header malformed for history depth {}",
                depth + 1
            );

            if depth == 0 {
                initial_state_commitment = Bytes32::from_array(*historical_header.state_root);

                // we should check cross-blocks pricing invariants, so EIP-1559 for gas price,
                // and check excess blob gas

                // EIP-1559
                {
                    assert_eq!(EIP_1559_ELASTICITY_MULTIPLIER, 2);
                    let parent_gas_limit = historical_header.gas_limit;
                    let parent_gas_target = parent_gas_limit >> 1; // EIP_1559_ELASTICITY_MULTIPLIER == 2

                    let parent_base_fee_per_gas = historical_header.base_fee_per_gas;
                    let parent_gas_used = historical_header.gas_used;

                    let slack = parent_gas_limit >> 10;
                    assert!(self.gas_limit < parent_gas_limit + slack);
                    assert!(self.gas_limit > parent_gas_limit - slack);

                    assert!(self.gas_limit >= EIP_1559_MIN_GAS_LIMIT);

                    let expected_base_fee_per_gas = if parent_gas_used == parent_gas_target {
                        parent_base_fee_per_gas
                    } else if parent_gas_used > parent_gas_target {
                        let gas_used_delta = parent_gas_used - parent_gas_target;
                        let base_fee_per_gas_delta = core::cmp::max(
                            parent_base_fee_per_gas * gas_used_delta
                                / parent_gas_target
                                / EIP_1559_BASE_FEE_MAX_CHANGE_DENOMINATOR,
                            1,
                        );
                        parent_base_fee_per_gas + base_fee_per_gas_delta
                    } else {
                        let gas_used_delta = parent_gas_target - parent_gas_used;
                        let base_fee_per_gas_delta = parent_base_fee_per_gas * gas_used_delta
                            / parent_gas_target
                            / EIP_1559_BASE_FEE_MAX_CHANGE_DENOMINATOR;
                        parent_base_fee_per_gas - base_fee_per_gas_delta
                    };
                    assert_eq!(expected_base_fee_per_gas, self.base_fee_per_gas);
                }

                // EIP-4844
                {
                    let t = historical_header.excess_blob_gas + historical_header.blob_gas_used;
                    let excess_blob_gas = t.saturating_sub(TARGET_BLOB_GAS_PER_BLOCK);
                    assert_eq!(self.excess_blob_gas, excess_blob_gas);
                }
            }
            parent_to_expect = Bytes32::from_array(*historical_header.parent_hash);
        }

        Ok(initial_state_commitment)
    }
}

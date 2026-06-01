//! TODO: this actually belongs to the bootloader, just for the ZK STF.
//! We will move it in future PRs.

use super::basic_metadata::{
    BasicBlockMetadata, BasicMetadata, BasicTransactionMetadata, ZkSpecificMetadata,
};
use super::chain_config::{ChainConfig, ChainConfigMetadata};
use crate::system::constants::*;
use crate::system::errors::internal::InternalError;
use crate::types_config::{EthereumIOTypesConfig, SystemIOTypesConfig};
use crate::utils::Bytes32;
use crate::{
    oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable},
    utils::exact_size_chain::{ExactSizeChain, ExactSizeChainN},
};
use ruint::aliases::{B160, U256};

pub struct ZkMetadata {
    pub block_level: BlockMetadataFromOracle,
    pub tx_level: TxLevelMetadata<EthereumIOTypesConfig>,
    pub chain_config: ChainConfig,
    pub _marker: core::marker::PhantomData<EthereumIOTypesConfig>,
}

#[derive(Clone, Debug, Default)]
pub struct TxLevelMetadata<IOTypes: SystemIOTypesConfig> {
    pub tx_origin: IOTypes::Address,
    pub tx_gas_price: U256,
    pub blobs: arrayvec::ArrayVec<Bytes32, { MAX_BLOBS_PER_TX }>,
    pub verified_fri_statements: arrayvec::ArrayVec<Bytes32, { MAX_FRI_STATEMENTS_PER_TX }>,
}

impl BasicTransactionMetadata<EthereumIOTypesConfig> for TxLevelMetadata<EthereumIOTypesConfig> {
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
    fn is_fri_statement_verified(&self, statement_versioned_hash: &Bytes32) -> bool {
        self.verified_fri_statements
            .contains(statement_versioned_hash)
    }
}

impl BasicBlockMetadata<EthereumIOTypesConfig> for ZkMetadata {
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

    fn coinbase(&self) -> B160 {
        self.block_level.coinbase()
    }

    fn block_gas_limit(&self) -> u64 {
        self.block_level.block_gas_limit()
    }

    fn individual_tx_gas_limit(&self) -> u64 {
        // EIP-7825: the per-tx gas cap is sourced from the static chain config
        // (committed into public input), not a compile-time feature flag.
        let block_gas_limit = self.block_level.block_gas_limit();
        let max_tx_gas_limit = self.chain_config.max_tx_gas_limit();
        if max_tx_gas_limit.is_enabled() {
            core::cmp::min(block_gas_limit, max_tx_gas_limit.value())
        } else {
            block_gas_limit
        }
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

impl BasicTransactionMetadata<EthereumIOTypesConfig> for ZkMetadata {
    fn tx_origin(&self) -> B160 {
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

    fn is_fri_statement_verified(&self, statement_versioned_hash: &Bytes32) -> bool {
        self.tx_level
            .is_fri_statement_verified(statement_versioned_hash)
    }
}

impl BasicMetadata<EthereumIOTypesConfig> for ZkMetadata {
    type TransactionMetadata = TxLevelMetadata<EthereumIOTypesConfig>;

    fn set_transaction_metadata(&mut self, tx_level_metadata: Self::TransactionMetadata) {
        self.tx_level = tx_level_metadata;
    }
}

impl ZkSpecificMetadata for ZkMetadata {
    fn get_pubdata_price(&self) -> U256 {
        self.block_level.pubdata_price
    }

    fn native_price(&self) -> U256 {
        self.block_level.native_price
    }

    fn get_pubdata_limit(&self) -> u64 {
        self.block_level.pubdata_limit
    }
}

impl ChainConfigMetadata for ZkMetadata {
    fn chain_config(&self) -> ChainConfig {
        self.chain_config
    }
}

pub const BLOCK_HASHES_WINDOW_SIZE: usize = 256;

/// Array of previous block hashes.
/// Hash for block number N will be at index [BLOCK_HASHES_WINDOW_SIZE - (current_block_number - N)]
/// (most recent will be at the end) if N is one of the most recent
/// BLOCK_HASHES_WINDOW_SIZE blocks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockHashes(pub [U256; BLOCK_HASHES_WINDOW_SIZE]);

impl Default for BlockHashes {
    fn default() -> Self {
        Self([U256::ZERO; BLOCK_HASHES_WINDOW_SIZE])
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for BlockHashes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.to_vec().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for BlockHashes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vec: Vec<U256> = Vec::deserialize(deserializer)?;
        let array: [U256; BLOCK_HASHES_WINDOW_SIZE] = vec
            .try_into()
            .map_err(|_| serde::de::Error::custom("Expected array of length 256"))?;
        Ok(Self(array))
    }
}

impl UsizeSerializable for BlockHashes {
    const USIZE_LEN: usize = <U256 as UsizeSerializable>::USIZE_LEN * BLOCK_HASHES_WINDOW_SIZE;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChainN::<_, _, BLOCK_HASHES_WINDOW_SIZE>::new(
            core::iter::empty::<usize>(),
            core::array::from_fn(|i| Some(self.0[i].iter())),
        )
    }
}

impl UsizeDeserializable for BlockHashes {
    const USIZE_LEN: usize = <U256 as UsizeDeserializable>::USIZE_LEN * BLOCK_HASHES_WINDOW_SIZE;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let mut hashes = [U256::ZERO; BLOCK_HASHES_WINDOW_SIZE];
        for hash in &mut hashes {
            *hash = U256::from_iter(src)?;
        }
        Ok(Self(hashes))
    }
}

// we only need to know limited set of parameters here,
// those that define "block", like uniform fee for block,
// block number, etc

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlockMetadataFromOracle {
    // Chain id is temporarily also added here (so that it can be easily passed from the oracle)
    // long term, we have to decide whether we want to keep it here, or add a separate oracle
    // type that would return some 'chain' specific metadata (as this class is supposed to hold block metadata only).
    pub chain_id: u64,
    pub block_number: u64,
    pub block_hashes: BlockHashes,
    pub timestamp: u64,
    pub eip1559_basefee: U256,
    pub pubdata_price: U256,
    pub native_price: U256,
    pub coinbase: B160,
    pub gas_limit: u64,
    pub pubdata_limit: u64,
    /// Source of randomness, currently holds the value
    /// of prevRandao.
    pub mix_hash: U256,
    pub blob_fee: U256,
}

impl BasicBlockMetadata<EthereumIOTypesConfig> for BlockMetadataFromOracle {
    fn chain_id(&self) -> u64 {
        self.chain_id
    }

    fn block_number(&self) -> u64 {
        self.block_number
    }

    fn block_historical_hash(&self, depth: u64) -> Option<Bytes32> {
        if depth >= 1 && depth <= BLOCK_HASHES_WINDOW_SIZE as u64 {
            let index = BLOCK_HASHES_WINDOW_SIZE as u64 - depth;
            Some(Bytes32::from_array(
                self.block_hashes.0[index as usize].to_be_bytes::<32>(),
            ))
        } else {
            None
        }
    }

    fn block_timestamp(&self) -> u64 {
        self.timestamp
    }

    fn block_randomness(&self) -> Option<Bytes32> {
        Some(Bytes32::from_array(self.mix_hash.to_be_bytes::<32>()))
    }

    fn coinbase(&self) -> B160 {
        self.coinbase
    }

    fn block_gas_limit(&self) -> u64 {
        self.gas_limit
    }

    fn individual_tx_gas_limit(&self) -> u64 {
        // The EIP-7825 per-tx gas cap is applied at the `ZkMetadata` level from
        // the static chain config. Block-level metadata reports the raw limit.
        self.gas_limit
    }

    fn eip1559_basefee(&self) -> U256 {
        self.eip1559_basefee
    }

    fn max_blobs(&self) -> usize {
        MAX_BLOBS_PER_BLOCK
    }

    fn blobs_gas_limit(&self) -> u64 {
        self.max_blobs() as u64 * GAS_PER_BLOB
    }

    fn blob_base_fee_per_gas(&self) -> U256 {
        self.blob_fee
    }
}

impl BlockMetadataFromOracle {
    pub fn new_for_test() -> Self {
        BlockMetadataFromOracle {
            eip1559_basefee: U256::from(1000u64),
            pubdata_price: U256::from(0u64),
            native_price: U256::from(10),
            block_number: 1,
            timestamp: 42,
            chain_id: 37,
            gas_limit: u64::MAX / 256,
            pubdata_limit: u64::MAX,
            coinbase: B160::ZERO,
            block_hashes: BlockHashes::default(),
            mix_hash: U256::ONE,
            blob_fee: U256::ZERO,
        }
    }
}

impl UsizeSerializable for BlockMetadataFromOracle {
    const USIZE_LEN: usize = <U256 as UsizeSerializable>::USIZE_LEN
        * (5 + BLOCK_HASHES_WINDOW_SIZE)
        + <u64 as UsizeSerializable>::USIZE_LEN * 5
        + <B160 as UsizeDeserializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            ExactSizeChain::new(
                ExactSizeChain::new(
                    ExactSizeChain::new(
                        ExactSizeChain::new(
                            ExactSizeChain::new(
                                ExactSizeChain::new(
                                    ExactSizeChain::new(
                                        ExactSizeChain::new(
                                            ExactSizeChain::new(
                                                ExactSizeChain::new(
                                                    UsizeSerializable::iter(&self.eip1559_basefee),
                                                    UsizeSerializable::iter(&self.pubdata_price),
                                                ),
                                                UsizeSerializable::iter(&self.native_price),
                                            ),
                                            UsizeSerializable::iter(&self.block_number),
                                        ),
                                        UsizeSerializable::iter(&self.timestamp),
                                    ),
                                    UsizeSerializable::iter(&self.chain_id),
                                ),
                                UsizeSerializable::iter(&self.gas_limit),
                            ),
                            UsizeSerializable::iter(&self.pubdata_limit),
                        ),
                        UsizeSerializable::iter(&self.coinbase),
                    ),
                    UsizeSerializable::iter(&self.block_hashes),
                ),
                UsizeSerializable::iter(&self.mix_hash),
            ),
            UsizeSerializable::iter(&self.blob_fee),
        )
    }
}

impl UsizeDeserializable for BlockMetadataFromOracle {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let eip1559_basefee = UsizeDeserializable::from_iter(src)?;
        let pubdata_price = UsizeDeserializable::from_iter(src)?;
        let native_price = UsizeDeserializable::from_iter(src)?;
        let block_number = UsizeDeserializable::from_iter(src)?;
        let timestamp = UsizeDeserializable::from_iter(src)?;
        let chain_id = UsizeDeserializable::from_iter(src)?;
        let gas_limit = UsizeDeserializable::from_iter(src)?;
        let pubdata_limit = UsizeDeserializable::from_iter(src)?;
        let coinbase = UsizeDeserializable::from_iter(src)?;
        let block_hashes = UsizeDeserializable::from_iter(src)?;
        let mix_hash = UsizeDeserializable::from_iter(src)?;
        let blob_fee = UsizeDeserializable::from_iter(src)?;

        let new = Self {
            eip1559_basefee,
            pubdata_price,
            native_price,
            block_number,
            timestamp,
            chain_id,
            gas_limit,
            pubdata_limit,
            coinbase,
            block_hashes,
            mix_hash,
            blob_fee,
        };

        Ok(new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize() {
        let original = BlockMetadataFromOracle::new_for_test();

        let serialized: Vec<usize> = original.iter().collect();
        let mut iter = serialized.into_iter();
        let deserialized = BlockMetadataFromOracle::from_iter(&mut iter).unwrap();

        assert_eq!(original, deserialized);
    }

    /// Pins the `is_fri_statement_verified` membership contract:
    /// finds hashes at any position in the per-tx list, and returns
    /// `false` for hashes not present.
    #[test]
    fn tx_metadata_accumulates_multiple_fri_statements() {
        let mut meta = TxLevelMetadata::<EthereumIOTypesConfig>::default();
        let h1 = Bytes32::from_array([1u8; 32]);
        let h2 = Bytes32::from_array([2u8; 32]);
        let h3 = Bytes32::from_array([3u8; 32]);

        assert!(!meta.is_fri_statement_verified(&h1));

        meta.verified_fri_statements.push(h1);
        meta.verified_fri_statements.push(h2);

        assert!(meta.is_fri_statement_verified(&h1));
        assert!(meta.is_fri_statement_verified(&h2));
        assert!(!meta.is_fri_statement_verified(&h3));
    }
}

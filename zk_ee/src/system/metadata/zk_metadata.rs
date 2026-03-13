//! TODO: this actually belongs to the bootloader, just for the ZK STF.
//! We will move it in future PRs.

use super::basic_metadata::{
    BasicBlockMetadata, BasicTransactionMetadata, ZkSpecificPricingMetadata,
};
use super::system_metadata::SystemMetadata;
use crate::system::constants::*;
use crate::system::errors::internal::InternalError;
use crate::types_config::{EthereumIOTypesConfig, SystemIOTypesConfig};
use crate::utils::Bytes32;
use crate::{
    oracle::word_serialization::{WordDeserializable, WordSerializable, WordSink},
};
use ruint::aliases::{B160, U256};

pub type ZkMetadata = SystemMetadata<
    EthereumIOTypesConfig,
    BlockMetadataFromOracle,
    TxLevelMetadata<EthereumIOTypesConfig>,
>;

#[derive(Clone, Debug, Default)]
pub struct TxLevelMetadata<IOTypes: SystemIOTypesConfig> {
    pub tx_origin: IOTypes::Address,
    pub tx_gas_price: U256,
    pub blobs: arrayvec::ArrayVec<Bytes32, { MAX_BLOBS_PER_BLOCK }>,
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

impl WordSerializable for BlockHashes {
    fn word_len(&self) -> usize {
        self.0.as_slice().iter().map(|hash| hash.word_len()).sum()
    }

    fn write_words(&self, out: &mut impl WordSink) {
        for hash in &self.0 {
            hash.write_words(out);
        }
    }
}

impl WordDeserializable for BlockHashes {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let mut hashes = [U256::ZERO; BLOCK_HASHES_WINDOW_SIZE];
        for hash in &mut hashes {
            *hash = U256::read_words(src)?;
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
        // Currently we don't have a separate individual tx gas limit,
        // so we return the block gas limit here.
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

impl ZkSpecificPricingMetadata for BlockMetadataFromOracle {
    fn get_pubdata_price(&self) -> U256 {
        self.pubdata_price
    }
    fn native_price(&self) -> U256 {
        self.native_price
    }
    fn get_pubdata_limit(&self) -> u64 {
        self.pubdata_limit
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

impl WordSerializable for BlockMetadataFromOracle {
    fn word_len(&self) -> usize {
        self.eip1559_basefee.word_len()
            + self.pubdata_price.word_len()
            + self.native_price.word_len()
            + self.block_number.word_len()
            + self.timestamp.word_len()
            + self.chain_id.word_len()
            + self.gas_limit.word_len()
            + self.pubdata_limit.word_len()
            + self.coinbase.word_len()
            + self.block_hashes.word_len()
            + self.mix_hash.word_len()
            + self.blob_fee.word_len()
    }

    fn write_words(&self, out: &mut impl WordSink) {
        self.eip1559_basefee.write_words(out);
        self.pubdata_price.write_words(out);
        self.native_price.write_words(out);
        self.block_number.write_words(out);
        self.timestamp.write_words(out);
        self.chain_id.write_words(out);
        self.gas_limit.write_words(out);
        self.pubdata_limit.write_words(out);
        self.coinbase.write_words(out);
        self.block_hashes.write_words(out);
        self.mix_hash.write_words(out);
        self.blob_fee.write_words(out);
    }
}

impl WordDeserializable for BlockMetadataFromOracle {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let eip1559_basefee = WordDeserializable::read_words(src)?;
        let pubdata_price = WordDeserializable::read_words(src)?;
        let native_price = WordDeserializable::read_words(src)?;
        let block_number = WordDeserializable::read_words(src)?;
        let timestamp = WordDeserializable::read_words(src)?;
        let chain_id = WordDeserializable::read_words(src)?;
        let gas_limit = WordDeserializable::read_words(src)?;
        let pubdata_limit = WordDeserializable::read_words(src)?;
        let coinbase = WordDeserializable::read_words(src)?;
        let block_hashes = WordDeserializable::read_words(src)?;
        let mix_hash = WordDeserializable::read_words(src)?;
        let blob_fee = WordDeserializable::read_words(src)?;

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

        let serialized = original.to_word_vec();
        let mut iter = serialized.into_iter();
        let deserialized = BlockMetadataFromOracle::read_words(&mut iter).unwrap();

        assert_eq!(original, deserialized);
    }
}

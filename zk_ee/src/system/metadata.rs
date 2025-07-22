use super::{
    errors::internal::InternalError,
    kv_markers::{ExactSizeChain, UsizeDeserializable, UsizeSerializable},
    types_config::SystemIOTypesConfig,
};
use ruint::aliases::{B160, U256};

#[derive(Clone, Copy, Debug, Default)]
pub struct Metadata<IOTypes: SystemIOTypesConfig> {
    pub chain_id: u64,
    pub tx_origin: IOTypes::Address,
    pub tx_gas_price: U256,
    pub block_level_metadata: BlockMetadataFromOracle,
}

/// Array of previous block hashes.
/// Hash for block number N will be at index [current_block_number - N - 1]
/// (most recent will be at the start) if N is one of the most recent
/// 256 blocks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockHashes(pub [U256; 256]);

impl Default for BlockHashes {
    fn default() -> Self {
        Self([U256::ZERO; 256])
    }
}

#[cfg(feature = "testing")]
impl serde::Serialize for BlockHashes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.to_vec().serialize(serializer)
    }
}

#[cfg(feature = "testing")]
impl<'de> serde::Deserialize<'de> for BlockHashes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vec: Vec<U256> = Vec::deserialize(deserializer)?;
        let array: [U256; 256] = vec
            .try_into()
            .map_err(|_| serde::de::Error::custom("Expected array of length 256"))?;
        Ok(Self(array))
    }
}

impl UsizeSerializable for BlockHashes {
    const USIZE_LEN: usize = <U256 as UsizeSerializable>::USIZE_LEN * 256;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        super::kv_markers::ExactSizeChainN::<_, _, 256>::new(
            core::iter::empty::<usize>(),
            core::array::from_fn(|i| Some(self.0[i].iter())),
        )
    }
}

impl UsizeDeserializable for BlockHashes {
    const USIZE_LEN: usize = <U256 as UsizeDeserializable>::USIZE_LEN * 256;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        Ok(Self(core::array::from_fn(|_| {
            U256::from_iter(src).unwrap_or_default()
        })))
    }
}

// we only need to know limited set of parameters here,
// those that define "block", like uniform fee for block,
// block number, etc

#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
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
    pub gas_per_pubdata: U256,
    pub native_price: U256,
    pub coinbase: B160,
    pub gas_limit: u64,
    /// Source of randomness, currently holds the value
    /// of prevRandao.
    pub mix_hash: U256,
}

impl BlockMetadataFromOracle {
    pub fn new_for_test() -> Self {
        BlockMetadataFromOracle {
            eip1559_basefee: U256::from(1000u64),
            gas_per_pubdata: U256::from(0u64),
            native_price: U256::from(10),
            block_number: 1,
            timestamp: 42,
            chain_id: 37,
            gas_limit: u64::MAX / 256,
            coinbase: B160::ZERO,
            block_hashes: BlockHashes::default(),
            mix_hash: U256::ONE,
        }
    }
}

impl UsizeSerializable for BlockMetadataFromOracle {
    const USIZE_LEN: usize = <U256 as UsizeSerializable>::USIZE_LEN * (4 + 256)
        + <u64 as UsizeSerializable>::USIZE_LEN * 4
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
                                            UsizeSerializable::iter(&self.eip1559_basefee),
                                            UsizeSerializable::iter(&self.gas_per_pubdata),
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
                    UsizeSerializable::iter(&self.coinbase),
                ),
                UsizeSerializable::iter(&self.block_hashes),
            ),
            UsizeSerializable::iter(&self.mix_hash),
        )
    }
}

impl UsizeDeserializable for BlockMetadataFromOracle {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let eip1559_basefee = UsizeDeserializable::from_iter(src)?;
        let gas_per_pubdata = UsizeDeserializable::from_iter(src)?;
        let native_price = UsizeDeserializable::from_iter(src)?;
        let block_number = UsizeDeserializable::from_iter(src)?;
        let timestamp = UsizeDeserializable::from_iter(src)?;
        let chain_id = UsizeDeserializable::from_iter(src)?;
        let gas_limit = UsizeDeserializable::from_iter(src)?;
        let coinbase = UsizeDeserializable::from_iter(src)?;
        let block_hashes = UsizeDeserializable::from_iter(src)?;
        let mix_hash = UsizeDeserializable::from_iter(src)?;

        let new = Self {
            eip1559_basefee,
            gas_per_pubdata,
            native_price,
            block_number,
            timestamp,
            chain_id,
            gas_limit,
            coinbase,
            block_hashes,
            mix_hash,
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
}

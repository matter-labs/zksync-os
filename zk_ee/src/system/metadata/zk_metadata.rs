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
    oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable},
    utils::exact_size_chain::{ExactSizeChain, ExactSizeChainN},
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

impl serde::Serialize for BlockHashes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.to_vec().serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for BlockHashes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vec: alloc::vec::Vec<U256> = alloc::vec::Vec::deserialize(deserializer)?;
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

unsafe impl<C: wincode::config::ConfigCore> wincode::SchemaWrite<C> for BlockHashes {
    type Src = Self;

    const TYPE_META: wincode::TypeMeta = wincode::TypeMeta::Static {
        size: BLOCK_HASHES_WINDOW_SIZE * 32,
        zero_copy: true,
    };

    fn size_of(_src: &Self) -> wincode::WriteResult<usize> {
        Ok(BLOCK_HASHES_WINDOW_SIZE * 32)
    }

    fn write(mut writer: impl wincode::io::Writer, src: &Self) -> wincode::WriteResult<()> {
        for item in &src.0 {
            <[u64; 4] as wincode::SchemaWrite<C>>::write(writer.by_ref(), item.as_limbs())?;
        }
        Ok(())
    }
}

unsafe impl<'de, C: wincode::config::ConfigCore> wincode::SchemaRead<'de, C> for BlockHashes {
    type Dst = Self;

    const TYPE_META: wincode::TypeMeta = wincode::TypeMeta::Static {
        size: BLOCK_HASHES_WINDOW_SIZE * 32,
        zero_copy: true,
    };

    fn read(
        mut reader: impl wincode::io::Reader<'de>,
        dst: &mut core::mem::MaybeUninit<Self>,
    ) -> wincode::ReadResult<()> {
        // SAFETY: U256 is #[repr(transparent)] over [u64; 4], so [U256; 256]
        // has the same layout as [u8; 8192] on LE. Bulk-read all 8192 bytes.
        unsafe { reader.copy_into_t(dst).map_err(wincode::ReadError::Io) }
    }
}

unsafe impl<C: wincode::config::ConfigCore> wincode::SchemaWrite<C> for BlockMetadataFromOracle {
    type Src = Self;

    fn size_of(src: &Self) -> wincode::WriteResult<usize> {
        let mut total = 0usize;
        total += <u64 as wincode::SchemaWrite<C>>::size_of(&src.chain_id)?;
        total += <u64 as wincode::SchemaWrite<C>>::size_of(&src.block_number)?;
        total += <BlockHashes as wincode::SchemaWrite<C>>::size_of(&src.block_hashes)?;
        total += <u64 as wincode::SchemaWrite<C>>::size_of(&src.timestamp)?;
        total += <[u64; 4] as wincode::SchemaWrite<C>>::size_of(src.eip1559_basefee.as_limbs())?;
        total += <[u64; 4] as wincode::SchemaWrite<C>>::size_of(src.pubdata_price.as_limbs())?;
        total += <[u64; 4] as wincode::SchemaWrite<C>>::size_of(src.native_price.as_limbs())?;
        total += <[u64; 3] as wincode::SchemaWrite<C>>::size_of(src.coinbase.as_limbs())?;
        total += <u64 as wincode::SchemaWrite<C>>::size_of(&src.gas_limit)?;
        total += <u64 as wincode::SchemaWrite<C>>::size_of(&src.pubdata_limit)?;
        total += <[u64; 4] as wincode::SchemaWrite<C>>::size_of(src.mix_hash.as_limbs())?;
        total += <[u64; 4] as wincode::SchemaWrite<C>>::size_of(src.blob_fee.as_limbs())?;
        Ok(total)
    }

    fn write(mut writer: impl wincode::io::Writer, src: &Self) -> wincode::WriteResult<()> {
        <u64 as wincode::SchemaWrite<C>>::write(writer.by_ref(), &src.chain_id)?;
        <u64 as wincode::SchemaWrite<C>>::write(writer.by_ref(), &src.block_number)?;
        <BlockHashes as wincode::SchemaWrite<C>>::write(writer.by_ref(), &src.block_hashes)?;
        <u64 as wincode::SchemaWrite<C>>::write(writer.by_ref(), &src.timestamp)?;
        <[u64; 4] as wincode::SchemaWrite<C>>::write(
            writer.by_ref(),
            src.eip1559_basefee.as_limbs(),
        )?;
        <[u64; 4] as wincode::SchemaWrite<C>>::write(
            writer.by_ref(),
            src.pubdata_price.as_limbs(),
        )?;
        <[u64; 4] as wincode::SchemaWrite<C>>::write(writer.by_ref(), src.native_price.as_limbs())?;
        <[u64; 3] as wincode::SchemaWrite<C>>::write(writer.by_ref(), src.coinbase.as_limbs())?;
        <u64 as wincode::SchemaWrite<C>>::write(writer.by_ref(), &src.gas_limit)?;
        <u64 as wincode::SchemaWrite<C>>::write(writer.by_ref(), &src.pubdata_limit)?;
        <[u64; 4] as wincode::SchemaWrite<C>>::write(writer.by_ref(), src.mix_hash.as_limbs())?;
        <[u64; 4] as wincode::SchemaWrite<C>>::write(writer.by_ref(), src.blob_fee.as_limbs())?;
        Ok(())
    }
}

unsafe impl<'de, C: wincode::config::ConfigCore> wincode::SchemaRead<'de, C>
    for BlockMetadataFromOracle
{
    type Dst = Self;

    fn read(
        mut reader: impl wincode::io::Reader<'de>,
        dst: &mut core::mem::MaybeUninit<Self>,
    ) -> wincode::ReadResult<()> {
        let chain_id = <u64 as wincode::SchemaRead<'de, C>>::get(reader.by_ref())?;
        let block_number = <u64 as wincode::SchemaRead<'de, C>>::get(reader.by_ref())?;
        let block_hashes = <BlockHashes as wincode::SchemaRead<'de, C>>::get(reader.by_ref())?;
        let timestamp = <u64 as wincode::SchemaRead<'de, C>>::get(reader.by_ref())?;

        let mut limbs4 = core::mem::MaybeUninit::<[u64; 4]>::uninit();
        <[u64; 4] as wincode::SchemaRead<'de, C>>::read(reader.by_ref(), &mut limbs4)?;
        let eip1559_basefee = U256::from_limbs(unsafe { limbs4.assume_init() });

        <[u64; 4] as wincode::SchemaRead<'de, C>>::read(reader.by_ref(), &mut limbs4)?;
        let pubdata_price = U256::from_limbs(unsafe { limbs4.assume_init() });

        <[u64; 4] as wincode::SchemaRead<'de, C>>::read(reader.by_ref(), &mut limbs4)?;
        let native_price = U256::from_limbs(unsafe { limbs4.assume_init() });

        let mut limbs3 = core::mem::MaybeUninit::<[u64; 3]>::uninit();
        <[u64; 3] as wincode::SchemaRead<'de, C>>::read(reader.by_ref(), &mut limbs3)?;
        let coinbase = B160::from_limbs(unsafe { limbs3.assume_init() });

        let gas_limit = <u64 as wincode::SchemaRead<'de, C>>::get(reader.by_ref())?;
        let pubdata_limit = <u64 as wincode::SchemaRead<'de, C>>::get(reader.by_ref())?;

        <[u64; 4] as wincode::SchemaRead<'de, C>>::read(reader.by_ref(), &mut limbs4)?;
        let mix_hash = U256::from_limbs(unsafe { limbs4.assume_init() });

        <[u64; 4] as wincode::SchemaRead<'de, C>>::read(reader.by_ref(), &mut limbs4)?;
        let blob_fee = U256::from_limbs(unsafe { limbs4.assume_init() });

        dst.write(Self {
            chain_id,
            block_number,
            block_hashes,
            timestamp,
            eip1559_basefee,
            pubdata_price,
            native_price,
            coinbase,
            gas_limit,
            pubdata_limit,
            mix_hash,
            blob_fee,
        });
        Ok(())
    }
}

// we only need to know limited set of parameters here,
// those that define "block", like uniform fee for block,
// block number, etc

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
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
}

use crate::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use crate::utils::exact_size_chain::ExactSizeChain;
use crate::{system::errors::internal::InternalError, types_config::SystemIOTypesConfig};
use wincode::config::ConfigCore;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "IOTypes::Address: serde::Serialize, IOTypes::StorageKey: serde::Serialize"
))]
#[serde(bound(
    deserialize = "IOTypes::Address: for<'a> serde::Deserialize<'a>, IOTypes::StorageKey: for<'a> serde::Deserialize<'a>"
))]
pub struct StorageAddress<IOTypes: SystemIOTypesConfig> {
    pub address: IOTypes::Address,
    pub key: IOTypes::StorageKey,
}

impl<IOTypes: SystemIOTypesConfig> UsizeSerializable for StorageAddress<IOTypes> {
    const USIZE_LEN: usize = <IOTypes::Address as UsizeSerializable>::USIZE_LEN
        + <IOTypes::StorageKey as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.address),
            UsizeSerializable::iter(&self.key),
        )
    }
}

impl<IOTypes: SystemIOTypesConfig> UsizeDeserializable for StorageAddress<IOTypes> {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let address = UsizeDeserializable::from_iter(src)?;
        let key = UsizeDeserializable::from_iter(src)?;
        Ok(Self { address, key })
    }
}

use crate::types_config::EthereumIOTypesConfig;
use crate::utils::Bytes32;
use ruint::aliases::B160;

/// Wincode serialization for `StorageAddress<EthereumIOTypesConfig>`.
/// B160 (address) is serialized as `[u64; 3]` (its limb representation).
/// Bytes32 (key) already has wincode impls.
unsafe impl<C: ConfigCore> wincode::SchemaWrite<C> for StorageAddress<EthereumIOTypesConfig> {
    type Src = Self;

    fn size_of(src: &Self) -> wincode::WriteResult<usize> {
        let mut total = 0usize;
        total += <[u64; 3] as wincode::SchemaWrite<C>>::size_of(src.address.as_limbs())?;
        total += <Bytes32 as wincode::SchemaWrite<C>>::size_of(&src.key)?;
        Ok(total)
    }

    fn write(mut writer: impl wincode::io::Writer, src: &Self) -> wincode::WriteResult<()> {
        <[u64; 3] as wincode::SchemaWrite<C>>::write(writer.by_ref(), src.address.as_limbs())?;
        <Bytes32 as wincode::SchemaWrite<C>>::write(writer.by_ref(), &src.key)?;
        Ok(())
    }
}

unsafe impl<'de, C: ConfigCore> wincode::SchemaRead<'de, C>
    for StorageAddress<EthereumIOTypesConfig>
{
    type Dst = Self;

    fn read(
        mut reader: impl wincode::io::Reader<'de>,
        dst: &mut core::mem::MaybeUninit<Self>,
    ) -> wincode::ReadResult<()> {
        let mut limbs = core::mem::MaybeUninit::<[u64; 3]>::uninit();
        <[u64; 3] as wincode::SchemaRead<'de, C>>::read(reader.by_ref(), &mut limbs)?;
        let address = B160::from_limbs(unsafe { limbs.assume_init() });
        let key = <Bytes32 as wincode::SchemaRead<'de, C>>::get(reader.by_ref())?;
        dst.write(Self { address, key });
        Ok(())
    }
}

use crate::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use crate::utils::exact_size_chain::ExactSizeChain;
use crate::{system::errors::internal::InternalError, types_config::SystemIOTypesConfig};
use wincode::config::ConfigCore;

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(bound(serialize = "IOTypes::StorageValue: serde::Serialize"))]
#[serde(bound(deserialize = "IOTypes::StorageValue: for<'a> serde::Deserialize<'a>"))]
pub struct InitialStorageSlotData<IOTypes: SystemIOTypesConfig> {
    // We need to know what was a value of the storage slot,
    // and whether it existed in the state or has to be created
    // (so additional information is needed to reconstruct creation location).
    pub is_new_storage_slot: bool,
    pub initial_value: IOTypes::StorageValue,
}

impl<IOTypes: SystemIOTypesConfig> UsizeSerializable for InitialStorageSlotData<IOTypes> {
    const USIZE_LEN: usize = <bool as UsizeSerializable>::USIZE_LEN
        + <IOTypes::StorageValue as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.is_new_storage_slot),
            UsizeSerializable::iter(&self.initial_value),
        )
    }
}

impl<IOTypes: SystemIOTypesConfig> UsizeDeserializable for InitialStorageSlotData<IOTypes> {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let is_new_storage_slot = UsizeDeserializable::from_iter(src)?;
        let initial_value = UsizeDeserializable::from_iter(src)?;
        Ok(Self {
            is_new_storage_slot,
            initial_value,
        })
    }
}

unsafe impl<C: ConfigCore, IOTypes: SystemIOTypesConfig> wincode::SchemaWrite<C>
    for InitialStorageSlotData<IOTypes>
where
    IOTypes::StorageValue: wincode::SchemaWrite<C, Src = IOTypes::StorageValue>,
{
    type Src = Self;

    fn size_of(src: &Self) -> wincode::WriteResult<usize> {
        let mut total = 0usize;
        total += <bool as wincode::SchemaWrite<C>>::size_of(&src.is_new_storage_slot)?;
        total += <IOTypes::StorageValue as wincode::SchemaWrite<C>>::size_of(&src.initial_value)?;
        Ok(total)
    }

    fn write(mut writer: impl wincode::io::Writer, src: &Self) -> wincode::WriteResult<()> {
        <bool as wincode::SchemaWrite<C>>::write(writer.by_ref(), &src.is_new_storage_slot)?;
        <IOTypes::StorageValue as wincode::SchemaWrite<C>>::write(
            writer.by_ref(),
            &src.initial_value,
        )?;
        Ok(())
    }
}

unsafe impl<'de, C: ConfigCore, IOTypes: SystemIOTypesConfig> wincode::SchemaRead<'de, C>
    for InitialStorageSlotData<IOTypes>
where
    IOTypes::StorageValue: wincode::SchemaRead<'de, C, Dst = IOTypes::StorageValue>,
{
    type Dst = Self;

    fn read(
        mut reader: impl wincode::io::Reader<'de>,
        dst: &mut core::mem::MaybeUninit<Self>,
    ) -> wincode::ReadResult<()> {
        let is_new_storage_slot = <bool as wincode::SchemaRead<'de, C>>::get(reader.by_ref())?;
        let initial_value =
            <IOTypes::StorageValue as wincode::SchemaRead<'de, C>>::get(reader.by_ref())?;
        dst.write(Self {
            is_new_storage_slot,
            initial_value,
        });
        Ok(())
    }
}

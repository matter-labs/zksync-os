use crate::oracle::usize_serialization::{
    UsizeDeserializable, UsizeSerializable, WordDeserializable, WordSerializable, WordSink,
};
use crate::utils::exact_size_chain::ExactSizeChain;
use crate::{system::errors::internal::InternalError, types_config::SystemIOTypesConfig};

#[derive(Clone, Copy, Debug, Default)]
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

impl<IOTypes: SystemIOTypesConfig> WordSerializable for InitialStorageSlotData<IOTypes> {
    fn word_len(&self) -> usize {
        self.is_new_storage_slot.word_len() + self.initial_value.word_len()
    }

    fn write_words(&self, out: &mut impl WordSink) {
        self.is_new_storage_slot.write_words(out);
        self.initial_value.write_words(out);
    }
}

impl<IOTypes: SystemIOTypesConfig> WordDeserializable for InitialStorageSlotData<IOTypes> {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let is_new_storage_slot = WordDeserializable::read_words(src)?;
        let initial_value = WordDeserializable::read_words(src)?;

        let new = Self {
            is_new_storage_slot,
            initial_value,
        };

        Ok(new)
    }
}

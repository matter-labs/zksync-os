use crate::oracle::usize_serialization::{
    UsizeDeserializable, UsizeSerializable, WordDeserializable, WordSerializable, WordSink,
};
use crate::utils::exact_size_chain::ExactSizeChain;
use crate::{system::errors::internal::InternalError, types_config::SystemIOTypesConfig};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

impl<IOTypes: SystemIOTypesConfig> WordSerializable for StorageAddress<IOTypes> {
    fn word_len(&self) -> usize {
        self.address.word_len() + self.key.word_len()
    }

    fn write_words(&self, out: &mut impl WordSink) {
        self.address.write_words(out);
        self.key.write_words(out);
    }
}

impl<IOTypes: SystemIOTypesConfig> WordDeserializable for StorageAddress<IOTypes> {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let address = WordDeserializable::read_words(src)?;
        let key = WordDeserializable::read_words(src)?;

        let new = Self { address, key };

        Ok(new)
    }
}

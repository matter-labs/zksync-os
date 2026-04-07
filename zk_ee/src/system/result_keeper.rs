///
/// This module contains definition of the result keeper trait.
///
/// Result keeper structure that will be called during execution to save the block execution result.
/// It's needed for sequencing(to collect receipts, diffs, pubdata).
///
/// Since we will not use it during the proving, we are using regular types, no need to serialize/deserialize.
///
use crate::common_structs::{
    GenericEventContentWithTxRef, GenericLogContentWithTxRef, PreimageType,
};
use crate::system::MAX_EVENT_TOPICS;
use crate::types_config::SystemIOTypesConfig;

pub trait IOResultKeeper<IOTypes: SystemIOTypesConfig> {
    fn events<'a>(
        &mut self,
        _iter: impl Iterator<Item = GenericEventContentWithTxRef<'a, MAX_EVENT_TOPICS, IOTypes>>,
    ) {
    }

    fn logs<'a>(&mut self, _iter: impl Iterator<Item = GenericLogContentWithTxRef<'a, IOTypes>>) {}

    fn storage_diffs(
        &mut self,
        _iter: impl Iterator<Item = (IOTypes::Address, IOTypes::StorageKey, IOTypes::StorageValue)>,
    ) {
    }

    fn basic_account_diffs(
        &mut self,
        _iter: impl Iterator<
            Item = (
                IOTypes::Address,
                (u64, IOTypes::NominalTokenValue, IOTypes::BytecodeHashValue),
            ),
        >,
    ) {
    }

    fn new_preimages<'a>(
        &mut self,
        _iter: impl Iterator<Item = (&'a IOTypes::BytecodeHashValue, &'a [u8], PreimageType)>,
    ) {
    }

    ///
    /// This method can be called several times with consecutive parts of pubdata.
    ///
    fn pubdata<'a>(&mut self, _value: &'a [u8]) {}

    /// Convenience if we will want to also dump account's properties encoding separately in raw form
    fn account_state_opaque_encoding(&mut self, _address: &IOTypes::Address, _encoding: &[u8]) {}
}

pub struct NopResultKeeper<T: 'static + Sized = ()> {
    _marker: core::marker::PhantomData<T>,
}

impl<T: 'static + Sized> Default for NopResultKeeper<T> {
    fn default() -> Self {
        Self {
            _marker: core::marker::PhantomData,
        }
    }
}

impl<T: 'static + Sized, IOTypes: SystemIOTypesConfig> IOResultKeeper<IOTypes>
    for NopResultKeeper<T>
{
}

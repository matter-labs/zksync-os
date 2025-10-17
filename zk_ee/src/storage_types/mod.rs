//! Serialization and deserialization helpers for keys and values for storage.

use arrayvec::ArrayVec;

use crate::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use crate::utils::exact_size_chain::{ExactSizeChain, ExactSizeChainN};

use super::system::errors::internal::InternalError;
use super::types_config::SystemIOTypesConfig;

// TODO(EVM-1167): cleanup

bitflags::bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct EmptyBitflags: u32 {}
}

pub trait ReadonlyKVMarker: 'static {
    const CAN_BE_COLD_AND_WARM_READ: bool = true;

    type Key: UsizeSerializable;
    type Value: UsizeDeserializable;
    type AccessStatsBitmask: bitflags::Flags<Bits = u32>;
}

pub trait ReadWriteKVMarker: ReadonlyKVMarker
where
    Self::Value: UsizeSerializable,
{
    const CAN_BE_COLD_AND_WARM_WRITE: bool = true;
}

// helper structs for most of the cases

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

        let new = Self { address, key };

        Ok(new)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InitialStorageSlotData<IOTypes: SystemIOTypesConfig> {
    // we need to know what was a value of the storage slot,
    // and whether it existed in the state or has to be created
    // (so additional information is needed to reconstruct creation location)
    pub is_new_storage_slot: bool,
    pub initial_value: IOTypes::StorageValue,
}

impl<IOTypes: SystemIOTypesConfig> Default for InitialStorageSlotData<IOTypes> {
    fn default() -> Self {
        Self {
            is_new_storage_slot: false,
            initial_value: IOTypes::StorageValue::default(),
        }
    }
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

        let new = Self {
            is_new_storage_slot,
            initial_value,
        };

        Ok(new)
    }
}

pub const MAX_EVENT_TOPICS: usize = 4;

pub struct EventFullKey<const N: usize, IOTypes: SystemIOTypesConfig> {
    pub address: IOTypes::Address,
    pub topics: ArrayVec<IOTypes::EventKey, N>,
}

impl<const N: usize, IOTypes: SystemIOTypesConfig> UsizeSerializable for EventFullKey<N, IOTypes> {
    const USIZE_LEN: usize =
        <IOTypes::Address as UsizeSerializable>::USIZE_LEN + IOTypes::EventKey::USIZE_LEN * N;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChainN::<_, _, N>::new(
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.address),
                core::iter::once(self.topics.len()),
            ),
            core::array::from_fn(|i| {
                let topic = self
                    .topics
                    .get(i)
                    .unwrap_or(IOTypes::static_default_event_key());
                Some(UsizeSerializable::iter(topic))
            }),
        )
    }
}

pub struct SignalFullKey<const N: usize, IOTypes: SystemIOTypesConfig> {
    pub address: IOTypes::Address,
    pub topics: ArrayVec<IOTypes::SignalingKey, N>,
}

impl<const N: usize, IOTypes: SystemIOTypesConfig> UsizeSerializable for SignalFullKey<N, IOTypes> {
    const USIZE_LEN: usize =
        <IOTypes::Address as UsizeSerializable>::USIZE_LEN + IOTypes::SignalingKey::USIZE_LEN * N;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChainN::<_, _, N>::new(
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.address),
                core::iter::once(self.topics.len()),
            ),
            core::array::from_fn(|i| {
                let topic = self
                    .topics
                    .get(i)
                    .unwrap_or(IOTypes::static_default_signaling_key());
                Some(UsizeSerializable::iter(topic))
            }),
        )
    }
}

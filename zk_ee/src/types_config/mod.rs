use crate::{
    oracle::usize_serialization::{WordDeserializable, WordSerializable},
    utils::Bytes32,
};

/// Trait to get the low u32 component
/// of an address, if this address fits into u32.
///
pub trait TryIntoLowAddress {
    fn try_into_low(&self) -> Option<u32>;
}

pub trait SystemIOTypesConfig: Sized + 'static + Send + Sync {
    // We want to define some associated types for addresses, storage keys, etc.
    // mainly for sizes. We also want to have those interpretable as byte sequences in general.
    type Address: WordSerializable
        + WordDeserializable
        + Clone
        + Copy
        + core::fmt::Debug
        + core::default::Default
        + TryIntoLowAddress;
    type StorageKey: WordSerializable
        + WordDeserializable
        + Clone
        + Copy
        + core::fmt::Debug
        + core::default::Default;
    type StorageValue: WordSerializable
        + WordDeserializable
        + Clone
        + Copy
        + core::fmt::Debug
        + core::default::Default;
    type NominalTokenValue: WordSerializable
        + WordDeserializable
        + Clone
        + Copy
        + core::fmt::Debug
        + core::default::Default;
    type BytecodeHashValue: WordSerializable
        + WordDeserializable
        + Clone
        + Copy
        + core::fmt::Debug
        + core::default::Default;
    // Events are something to be consumed only in the system itself, and it'll never get passed
    // to the outside environment
    type EventKey: WordSerializable + Clone + Copy + core::fmt::Debug + core::default::Default;
    // Signals can be passed to outside environments (like L2 to L1 messages)
    type SignalingKey: WordSerializable + Clone + Copy + core::fmt::Debug + core::default::Default;

    // // and in general under address info we want to have some data
    // type AddressSpecificInfo: WordSerializable + WordDeserializable;

    fn static_default_event_key() -> &'static Self::EventKey;
    fn static_default_signaling_key() -> &'static Self::SignalingKey;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct EthereumIOTypesConfig;

use ruint::aliases::*;

impl TryIntoLowAddress for B160 {
    fn try_into_low(&self) -> Option<u32> {
        let limbs = self.as_limbs();
        let lo = limbs[0];

        // low limb must fit in u32, and all higher limbs must be zero
        if lo <= u32::MAX as _ && limbs[1..].iter().all(|&w| w == 0) {
            Some(lo as u32)
        } else {
            None
        }
    }
}

impl SystemIOTypesConfig for EthereumIOTypesConfig {
    type Address = B160;
    type StorageKey = Bytes32;
    type StorageValue = Bytes32;
    type NominalTokenValue = U256;
    type BytecodeHashValue = Bytes32;
    type EventKey = Bytes32;
    type SignalingKey = Bytes32;

    fn static_default_event_key() -> &'static Self::EventKey {
        &Bytes32::ZERO
    }

    fn static_default_signaling_key() -> &'static Self::SignalingKey {
        &Bytes32::ZERO
    }
}

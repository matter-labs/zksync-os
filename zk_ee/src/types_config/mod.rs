use crate::kv_markers::*;
use crate::utils::Bytes32;

pub trait SystemIOTypesConfig: Sized + 'static + Send + Sync {
    // We want to define some associated types for addresses, storage keys, etc.
    // mainly for sizes. We also want to have those interpretable as byte sequences in general.
    type Address: UsizeSerializable
        + UsizeDeserializable
        + Clone
        + core::fmt::Debug
        + core::default::Default;
    type StorageKey: UsizeSerializable
        + UsizeDeserializable
        + Clone
        + core::fmt::Debug
        + core::default::Default;
    type StorageValue: UsizeSerializable
        + UsizeDeserializable
        + Clone
        + core::fmt::Debug
        + core::default::Default;
    type NominalTokenValue: UsizeSerializable
        + UsizeDeserializable
        + Clone
        + core::fmt::Debug
        + core::default::Default;
    type BytecodeHashValue: UsizeSerializable
        + UsizeDeserializable
        + Clone
        + core::fmt::Debug
        + core::default::Default;
    // Events are something to be consumed only in the system itself, and it'll never get passed
    // to the outside environment
    type EventKey: UsizeSerializable + Clone + core::fmt::Debug + core::default::Default;
    // Signals can be passed to outside environments (like L2 to L1 messages)
    type SignalingKey: UsizeSerializable + Clone + core::fmt::Debug + core::default::Default;

    fn static_default_event_key() -> &'static Self::EventKey;
    fn static_default_signaling_key() -> &'static Self::SignalingKey;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct EthereumIOTypesConfig;

impl SystemIOTypesConfig for EthereumIOTypesConfig {
    type Address = ruint::aliases::B160;
    type StorageKey = Bytes32;
    type StorageValue = Bytes32;
    type NominalTokenValue = u256::U256;
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

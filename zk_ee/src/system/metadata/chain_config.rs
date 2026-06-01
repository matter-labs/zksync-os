use crate::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use crate::system::errors::internal::InternalError;
use crate::{internal_error, utils::exact_size_chain::ExactSizeChain};

/// Default EIP-170 deployed contract code-size limit.
pub const DEFAULT_MAX_CONTRACT_SIZE: u32 = 0x6000;

/// Default EIP-7825 single-transaction gas limit (2^24).
pub const DEFAULT_MAX_TX_GAS_LIMIT: u64 = 1 << 24;

/// Generates a width-specific configurable limit type.
///
/// A configurable limit can be switched off at the chain configuration level.
/// Disabled limits are canonicalized as `(enabled = false, value = 0)`. This
/// prevents two semantically equivalent disabled limits from producing
/// different public-input preimages.
macro_rules! configurable_limit {
    ($name:ident, $raw:ident, $ty:ty, $err:literal) => {
        #[cfg_attr(feature = "serde", derive(serde::Serialize))]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name {
            enabled: bool,
            value: $ty,
        }

        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                #[derive(serde::Deserialize)]
                struct $raw {
                    enabled: bool,
                    value: $ty,
                }

                let raw = <$raw as serde::Deserialize>::deserialize(deserializer)?;
                let limit = Self {
                    enabled: raw.enabled,
                    value: raw.value,
                };
                limit
                    .validate()
                    .map_err(|_| serde::de::Error::custom($err))?;

                Ok(limit)
            }
        }

        impl $name {
            /// Constructs a trusted in-code limit and canonicalizes disabled
            /// values to zero. Untrusted input must use deserialization, which
            /// rejects non-canonical disabled values.
            pub const fn new(enabled: bool, value: $ty) -> Self {
                if enabled {
                    Self { enabled, value }
                } else {
                    Self {
                        enabled: false,
                        value: 0,
                    }
                }
            }

            pub const fn enabled(value: $ty) -> Self {
                Self {
                    enabled: true,
                    value,
                }
            }

            pub const fn disabled() -> Self {
                Self {
                    enabled: false,
                    value: 0,
                }
            }

            pub const fn is_enabled(&self) -> bool {
                self.enabled
            }

            pub const fn value(&self) -> $ty {
                self.value
            }

            pub fn validate(&self) -> Result<(), InternalError> {
                if !self.enabled && self.value != 0 {
                    return Err(internal_error!($err));
                }

                Ok(())
            }

            pub fn is_satisfied_by(&self, value: $ty) -> bool {
                !self.enabled || value <= self.value
            }
        }

        impl UsizeSerializable for $name {
            const USIZE_LEN: usize =
                <bool as UsizeSerializable>::USIZE_LEN + <$ty as UsizeSerializable>::USIZE_LEN;

            fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
                ExactSizeChain::new(
                    UsizeSerializable::iter(&self.enabled),
                    UsizeSerializable::iter(&self.value),
                )
            }
        }

        impl UsizeDeserializable for $name {
            const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

            fn from_iter(
                src: &mut impl ExactSizeIterator<Item = usize>,
            ) -> Result<Self, InternalError> {
                let enabled = UsizeDeserializable::from_iter(src)?;
                let value = UsizeDeserializable::from_iter(src)?;
                let limit = Self { enabled, value };
                limit.validate()?;

                Ok(limit)
            }
        }
    };
}

configurable_limit!(
    ConfigurableLimitU32,
    RawConfigurableLimitU32,
    u32,
    "disabled configurable limit must have zero value"
);
configurable_limit!(
    ConfigurableLimitU64,
    RawConfigurableLimitU64,
    u64,
    "disabled configurable limit must have zero value"
);

/// Static chain-level execution rules committed into the batch public input.
///
/// Changing this value is a protocol-upgrade boundary and batches must not span
/// configurations.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainConfig {
    fri_proof_verification_enabled: bool,
    /// Optional deployed bytecode size limit. This does not configure the
    /// separate EIP-3860 initcode-size limit.
    max_contract_size: ConfigurableLimitU32,
    /// Optional EIP-7825 single-transaction gas limit. When enabled, the
    /// effective per-tx limit is `min(block_gas_limit, value)`.
    max_tx_gas_limit: ConfigurableLimitU64,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ChainConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct RawChainConfig {
            fri_proof_verification_enabled: bool,
            max_contract_size: ConfigurableLimitU32,
            // Defaults to the behavior-preserving EIP-7825 cap so that older
            // dumps without this field deserialize to current behavior.
            #[serde(default = "default_max_tx_gas_limit")]
            max_tx_gas_limit: ConfigurableLimitU64,
        }

        let raw = <RawChainConfig as serde::Deserialize>::deserialize(deserializer)?;
        let config = Self {
            fri_proof_verification_enabled: raw.fri_proof_verification_enabled,
            max_contract_size: raw.max_contract_size,
            max_tx_gas_limit: raw.max_tx_gas_limit,
        };
        config
            .validate()
            .map_err(|_| serde::de::Error::custom("invalid chain config"))?;

        Ok(config)
    }
}

#[cfg(feature = "serde")]
fn default_max_tx_gas_limit() -> ConfigurableLimitU64 {
    ConfigurableLimitU64::enabled(DEFAULT_MAX_TX_GAS_LIMIT)
}

impl ChainConfig {
    pub fn new(
        fri_proof_verification_enabled: bool,
        max_contract_size: ConfigurableLimitU32,
        max_tx_gas_limit: ConfigurableLimitU64,
    ) -> Result<Self, InternalError> {
        let config = Self {
            fri_proof_verification_enabled,
            max_contract_size,
            max_tx_gas_limit,
        };
        config.validate()?;

        Ok(config)
    }

    pub const fn default_for_chain() -> Self {
        Self {
            fri_proof_verification_enabled: false,
            max_contract_size: ConfigurableLimitU32::enabled(DEFAULT_MAX_CONTRACT_SIZE),
            max_tx_gas_limit: ConfigurableLimitU64::enabled(DEFAULT_MAX_TX_GAS_LIMIT),
        }
    }

    pub const fn with_fri_proof_verification_enabled(mut self, enabled: bool) -> Self {
        self.fri_proof_verification_enabled = enabled;
        self
    }

    pub const fn fri_proof_verification_enabled(&self) -> bool {
        self.fri_proof_verification_enabled
    }

    pub const fn max_contract_size(&self) -> ConfigurableLimitU32 {
        self.max_contract_size
    }

    pub const fn max_tx_gas_limit(&self) -> ConfigurableLimitU64 {
        self.max_tx_gas_limit
    }

    pub fn validate(&self) -> Result<(), InternalError> {
        self.max_contract_size.validate()?;
        self.max_tx_gas_limit.validate()
    }
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self::default_for_chain()
    }
}

impl UsizeSerializable for ChainConfig {
    const USIZE_LEN: usize = <bool as UsizeSerializable>::USIZE_LEN
        + <ConfigurableLimitU32 as UsizeSerializable>::USIZE_LEN
        + <ConfigurableLimitU64 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.fri_proof_verification_enabled),
                UsizeSerializable::iter(&self.max_contract_size),
            ),
            UsizeSerializable::iter(&self.max_tx_gas_limit),
        )
    }
}

impl UsizeDeserializable for ChainConfig {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let fri_proof_verification_enabled = UsizeDeserializable::from_iter(src)?;
        let max_contract_size = UsizeDeserializable::from_iter(src)?;
        let max_tx_gas_limit = UsizeDeserializable::from_iter(src)?;

        let config = Self {
            fri_proof_verification_enabled,
            max_contract_size,
            max_tx_gas_limit,
        };
        config.validate()?;

        Ok(config)
    }
}

/// Metadata types that expose static chain-level execution configuration.
pub trait ChainConfigMetadata {
    fn chain_config(&self) -> ChainConfig {
        ChainConfig::default()
    }

    fn fri_proof_verification_enabled(&self) -> bool {
        self.chain_config().fri_proof_verification_enabled()
    }

    fn max_contract_size(&self) -> ConfigurableLimitU32 {
        self.chain_config().max_contract_size()
    }

    fn max_tx_gas_limit(&self) -> ConfigurableLimitU64 {
        self.chain_config().max_tx_gas_limit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_limit_must_be_canonical() {
        assert_eq!(ConfigurableLimitU32::new(false, 1).value(), 0);
        assert_eq!(ConfigurableLimitU32::disabled().value(), 0);
        assert_eq!(ConfigurableLimitU64::new(false, 1).value(), 0);
        assert_eq!(ConfigurableLimitU64::disabled().value(), 0);
    }

    #[test]
    fn chain_config_roundtrips_through_usize_serialization() {
        let original = ChainConfig::default_for_chain().with_fri_proof_verification_enabled(true);
        let serialized: Vec<usize> = original.iter().collect();
        let mut iter = serialized.into_iter();
        let deserialized = ChainConfig::from_iter(&mut iter).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn chain_config_usize_deserialization_rejects_non_canonical_limit() {
        // fri flag, then a non-canonical disabled contract-size limit.
        let mut serialized = vec![false as usize, false as usize, 10usize].into_iter();

        assert!(ChainConfig::from_iter(&mut serialized).is_err());
    }

    #[test]
    fn disabled_limit_deserialization_rejects_non_canonical_value() {
        let mut serialized = vec![false as usize, 10usize].into_iter();
        assert!(ConfigurableLimitU32::from_iter(&mut serialized).is_err());

        let mut serialized = vec![false as usize, 10usize].into_iter();
        assert!(ConfigurableLimitU64::from_iter(&mut serialized).is_err());
    }
}

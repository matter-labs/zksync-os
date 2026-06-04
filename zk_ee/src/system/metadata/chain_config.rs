use crate::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use crate::system::errors::internal::InternalError;
use crate::utils::exact_size_chain::ExactSizeChain;

/// EIP-170 deployed contract code-size limit.
pub const DEFAULT_MAX_CONTRACT_SIZE: u32 = 0x6000;

/// EIP-3860 initcode-size limit, twice the deployed code-size limit.
pub const DEFAULT_MAX_INITCODE_SIZE: u32 = DEFAULT_MAX_CONTRACT_SIZE * 2;

/// Default EIP-7825 single-transaction gas limit (2^24).
pub const DEFAULT_MAX_TX_GAS_LIMIT: u64 = 1 << 24;

/// Static chain-level execution rules committed into the batch public input.
///
/// Changing this value is a protocol-upgrade boundary and batches must not span
/// configurations.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainConfig {
    fri_proof_verification_enabled: bool,
    /// EIP-7825 single-transaction gas limit. The effective per-tx limit is
    /// `min(block_gas_limit, max_tx_gas_limit)`.
    #[cfg_attr(feature = "serde", serde(default = "default_max_tx_gas_limit"))]
    max_tx_gas_limit: u64,
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
            // Defaults to the behavior-preserving EIP-7825 cap so that older
            // dumps without this field deserialize to current behavior.
            #[serde(default = "default_max_tx_gas_limit")]
            max_tx_gas_limit: u64,
        }

        let raw = <RawChainConfig as serde::Deserialize>::deserialize(deserializer)?;
        let config = Self {
            fri_proof_verification_enabled: raw.fri_proof_verification_enabled,
            max_tx_gas_limit: raw.max_tx_gas_limit,
        };
        config
            .validate()
            .map_err(|_| serde::de::Error::custom("invalid chain config"))?;

        Ok(config)
    }
}

#[cfg(feature = "serde")]
fn default_max_tx_gas_limit() -> u64 {
    DEFAULT_MAX_TX_GAS_LIMIT
}

impl ChainConfig {
    pub fn new(
        fri_proof_verification_enabled: bool,
        max_tx_gas_limit: u64,
    ) -> Result<Self, InternalError> {
        let config = Self {
            fri_proof_verification_enabled,
            max_tx_gas_limit,
        };
        config.validate()?;

        Ok(config)
    }

    pub const fn default_for_chain() -> Self {
        Self {
            fri_proof_verification_enabled: false,
            max_tx_gas_limit: DEFAULT_MAX_TX_GAS_LIMIT,
        }
    }

    pub const fn with_fri_proof_verification_enabled(mut self, enabled: bool) -> Self {
        self.fri_proof_verification_enabled = enabled;
        self
    }

    pub const fn fri_proof_verification_enabled(&self) -> bool {
        self.fri_proof_verification_enabled
    }

    pub const fn max_tx_gas_limit(&self) -> u64 {
        self.max_tx_gas_limit
    }

    pub fn validate(&self) -> Result<(), InternalError> {
        Ok(())
    }
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self::default_for_chain()
    }
}

impl UsizeSerializable for ChainConfig {
    const USIZE_LEN: usize =
        <bool as UsizeSerializable>::USIZE_LEN + <u64 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.fri_proof_verification_enabled),
            UsizeSerializable::iter(&self.max_tx_gas_limit),
        )
    }
}

impl UsizeDeserializable for ChainConfig {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let fri_proof_verification_enabled = UsizeDeserializable::from_iter(src)?;
        let max_tx_gas_limit = UsizeDeserializable::from_iter(src)?;

        let config = Self {
            fri_proof_verification_enabled,
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

    fn max_tx_gas_limit(&self) -> u64 {
        self.chain_config().max_tx_gas_limit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_config_roundtrips_through_usize_serialization() {
        let original = ChainConfig::default_for_chain().with_fri_proof_verification_enabled(true);
        let serialized: Vec<usize> = original.iter().collect();
        let mut iter = serialized.into_iter();
        let deserialized = ChainConfig::from_iter(&mut iter).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn chain_config_new_sets_all_fields() {
        let config = ChainConfig::new(true, DEFAULT_MAX_TX_GAS_LIMIT + 1).unwrap();

        assert!(config.fri_proof_verification_enabled());
        assert_eq!(config.max_tx_gas_limit(), DEFAULT_MAX_TX_GAS_LIMIT + 1);
    }
}

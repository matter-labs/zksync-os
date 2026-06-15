use crate::internal_error;
use crate::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use crate::oracle::{query_ids::CHAIN_CONFIG_QUERY_ID, IOOracle};
use crate::system::errors::internal::InternalError;
use crate::utils::exact_size_chain::ExactSizeChain;

use super::basic_metadata::ChainConfigMetadata;

/// EIP-7825 single-transaction gas limit (2^24). This is both the default
/// per-tx gas cap and the lower bound for any chain-configured value: a chain
/// may raise the cap above Ethereum's limit but must not set it below.
pub const DEFAULT_MAX_TX_GAS_LIMIT: u64 = 1 << 24;

/// Chain-level execution rules committed into the batch public input.
///
/// These values are fixed for the duration of a batch (a batch must not span
/// different configurations), but they are not immutable: they can change
/// between batches via, e.g., a migration (`fri_proof_verification_enabled`)
/// or a chain admin action (`max_tx_gas_limit`).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainConfig {
    /// Chain id. This is a static chain-level rule, so it lives here rather
    /// than in per-block metadata.
    chain_id: u64,
    fri_proof_verification_enabled: bool,
    /// EIP-7825 single-transaction gas limit. The effective per-tx limit is
    /// `min(block_gas_limit, max_tx_gas_limit)`.
    // Defaults to the behavior-preserving EIP-7825 cap so that older dumps
    // without this field deserialize to current behavior.
    #[cfg_attr(feature = "serde", serde(default = "default_max_tx_gas_limit"))]
    max_tx_gas_limit: u64,
}

#[cfg(feature = "serde")]
fn default_max_tx_gas_limit() -> u64 {
    DEFAULT_MAX_TX_GAS_LIMIT
}

impl ChainConfig {
    /// Reads the run-frozen chain config from the oracle. Sourced once per run
    /// and reused by execution and public-input construction. Deserialization
    /// is a pure parse; the limitation is enforced separately via [`Self::validate`].
    pub fn read_from_oracle(oracle: &mut impl IOOracle) -> Result<Self, InternalError> {
        oracle.query_with_empty_input(CHAIN_CONFIG_QUERY_ID)
    }

    pub fn new(
        chain_id: u64,
        fri_proof_verification_enabled: bool,
        max_tx_gas_limit: u64,
    ) -> Result<Self, InternalError> {
        let config = Self {
            chain_id,
            fri_proof_verification_enabled,
            max_tx_gas_limit,
        };
        config.validate()?;

        Ok(config)
    }

    /// Canonical default configuration: chain id `0`, FRI proof verification
    /// off, and the per-tx gas cap at the EIP-7825 limit. This is the `const`
    /// equivalent of [`Default::default`] (which forwards here) and is the
    /// default used by the forward-run entrypoints; callers that need specific
    /// values construct via [`ChainConfig::new`].
    pub const fn default_for_chain() -> Self {
        Self {
            chain_id: 0,
            fri_proof_verification_enabled: false,
            max_tx_gas_limit: DEFAULT_MAX_TX_GAS_LIMIT,
        }
    }

    pub const fn chain_id(&self) -> u64 {
        self.chain_id
    }

    pub const fn fri_proof_verification_enabled(&self) -> bool {
        self.fri_proof_verification_enabled
    }

    pub const fn max_tx_gas_limit(&self) -> u64 {
        self.max_tx_gas_limit
    }

    /// Checks chain-level limitations on the config. This is enforced at the
    /// system boundary (when the config is loaded for block execution), not
    /// during (de)serialization, so deserialization stays a pure parse.
    pub fn validate(&self) -> Result<(), InternalError> {
        // The per-tx gas cap must not be configured below Ethereum's EIP-7825
        // single-transaction gas limit; a chain may only raise it.
        if self.max_tx_gas_limit < DEFAULT_MAX_TX_GAS_LIMIT {
            return Err(internal_error!(
                "max_tx_gas_limit must be at least the EIP-7825 single-tx gas limit"
            ));
        }
        Ok(())
    }
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self::default_for_chain()
    }
}

impl UsizeSerializable for ChainConfig {
    const USIZE_LEN: usize = <u64 as UsizeSerializable>::USIZE_LEN
        + <bool as UsizeSerializable>::USIZE_LEN
        + <u64 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.chain_id),
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.fri_proof_verification_enabled),
                UsizeSerializable::iter(&self.max_tx_gas_limit),
            ),
        )
    }
}

impl UsizeDeserializable for ChainConfig {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let chain_id = UsizeDeserializable::from_iter(src)?;
        let fri_proof_verification_enabled = UsizeDeserializable::from_iter(src)?;
        let max_tx_gas_limit = UsizeDeserializable::from_iter(src)?;

        Ok(Self {
            chain_id,
            fri_proof_verification_enabled,
            max_tx_gas_limit,
        })
    }
}

impl ChainConfigMetadata for ChainConfig {
    fn chain_config(&self) -> ChainConfig {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_config_roundtrips_through_usize_serialization() {
        let original = ChainConfig::new(37, true, DEFAULT_MAX_TX_GAS_LIMIT).unwrap();
        let serialized: Vec<usize> = original.iter().collect();
        let mut iter = serialized.into_iter();
        let deserialized = ChainConfig::from_iter(&mut iter).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn chain_config_new_sets_all_fields() {
        let config = ChainConfig::new(37, true, DEFAULT_MAX_TX_GAS_LIMIT + 1).unwrap();

        assert_eq!(config.chain_id(), 37);
        assert!(config.fri_proof_verification_enabled());
        assert_eq!(config.max_tx_gas_limit(), DEFAULT_MAX_TX_GAS_LIMIT + 1);
    }

    #[test]
    fn chain_config_accepts_max_tx_gas_limit_at_eip7825_floor() {
        assert!(ChainConfig::new(37, false, DEFAULT_MAX_TX_GAS_LIMIT).is_ok());
    }

    #[test]
    fn chain_config_rejects_max_tx_gas_limit_below_eip7825_floor() {
        assert!(ChainConfig::new(37, false, DEFAULT_MAX_TX_GAS_LIMIT - 1).is_err());
    }

    #[test]
    fn usize_deserialization_does_not_validate() {
        // Validation is enforced at the system boundary, not during
        // deserialization, so a below-floor value parses successfully and is
        // only rejected by an explicit `validate()`.
        let mut serialized: Vec<usize> = ChainConfig::default_for_chain().iter().collect();
        // Last field is max_tx_gas_limit; drop it below the floor.
        *serialized.last_mut().unwrap() = (DEFAULT_MAX_TX_GAS_LIMIT - 1) as usize;
        let mut iter = serialized.into_iter();

        let config = ChainConfig::from_iter(&mut iter).expect("deserialization must not validate");
        assert!(config.validate().is_err());
    }
}

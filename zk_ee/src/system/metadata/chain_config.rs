use crate::common_structs::da_commitment_scheme::PubdataContent;
use crate::internal_error;
use crate::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use crate::oracle::{query_ids::CHAIN_CONFIG_QUERY_ID, IOOracle};
use crate::system::errors::internal::InternalError;
use crate::utils::exact_size_chain::ExactSizeChain;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use ruint::aliases::U256;

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
    /// Data availability mode: whether the batch commits the full pubdata
    /// (`FullPubdata`) or only the mandatory L2->L1 log section (`LogsOnly`).
    // Defaults to `FullPubdata` (commit everything) so that older dumps without this
    // field deserialize to the behavior-preserving choice.
    #[cfg_attr(feature = "serde", serde(default = "default_pubdata_content"))]
    pubdata_content: PubdataContent,
}

#[cfg(feature = "serde")]
fn default_max_tx_gas_limit() -> u64 {
    DEFAULT_MAX_TX_GAS_LIMIT
}

#[cfg(feature = "serde")]
fn default_pubdata_content() -> PubdataContent {
    PubdataContent::FullPubdata
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
            pubdata_content: PubdataContent::FullPubdata,
        };
        config.validate()?;

        Ok(config)
    }

    /// Returns the config with the given pubdata content set. Chained after [`Self::new`]
    /// (which defaults to [`PubdataContent::FullPubdata`]) for validium chains.
    pub const fn with_pubdata_content(mut self, pubdata_content: PubdataContent) -> Self {
        self.pubdata_content = pubdata_content;
        self
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
            pubdata_content: PubdataContent::FullPubdata,
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

    pub const fn pubdata_content(&self) -> PubdataContent {
        self.pubdata_content
    }

    /// Canonical keccak256 commitment to the chain config.
    ///
    /// Committed into the batch public input so that the public-input layout
    /// stays fixed (a single 32-byte word) even as the config's field set
    /// evolves. Encoding, in order:
    /// - `chain_id`: uint256 big-endian (32-byte word)
    /// - `fri_proof_verification_enabled`: 32-byte word, last byte `0`/`1`
    /// - `max_tx_gas_limit`: uint64 big-endian, right-aligned in a 32-byte word
    /// - `pubdata_content`: 32-byte word, last byte the mode id (`FullPubdata=0`/`LogsOnly=1`)
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = Keccak256::new();
        hasher.update(U256::from(self.chain_id).to_be_bytes::<32>());
        let mut fri_word = [0u8; 32];
        fri_word[31] = u8::from(self.fri_proof_verification_enabled);
        hasher.update(fri_word);
        let mut gas_word = [0u8; 32];
        gas_word[24..].copy_from_slice(&self.max_tx_gas_limit.to_be_bytes());
        hasher.update(gas_word);
        let mut pubdata_content_word = [0u8; 32];
        pubdata_content_word[31] = self.pubdata_content as u8;
        hasher.update(pubdata_content_word);
        hasher.finalize()
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
        + <u64 as UsizeSerializable>::USIZE_LEN
        + <PubdataContent as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.chain_id),
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.fri_proof_verification_enabled),
                ExactSizeChain::new(
                    UsizeSerializable::iter(&self.max_tx_gas_limit),
                    UsizeSerializable::iter(&self.pubdata_content),
                ),
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
        let pubdata_content = UsizeDeserializable::from_iter(src)?;

        Ok(Self {
            chain_id,
            fri_proof_verification_enabled,
            max_tx_gas_limit,
            pubdata_content,
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
        // `new` defaults to `FullPubdata`; `LogsOnly` is opted into via `with_pubdata_content`.
        assert_eq!(config.pubdata_content(), PubdataContent::FullPubdata);
    }

    #[test]
    fn chain_config_with_pubdata_content_sets_validium_and_roundtrips() {
        let config = ChainConfig::new(37, false, DEFAULT_MAX_TX_GAS_LIMIT)
            .unwrap()
            .with_pubdata_content(PubdataContent::LogsOnly);
        assert_eq!(config.pubdata_content(), PubdataContent::LogsOnly);

        let serialized: Vec<usize> = config.iter().collect();
        let mut iter = serialized.into_iter();
        assert_eq!(ChainConfig::from_iter(&mut iter).unwrap(), config);
    }

    #[test]
    fn chain_config_hash_commits_to_pubdata_content() {
        let full_pubdata = ChainConfig::new(37, false, DEFAULT_MAX_TX_GAS_LIMIT).unwrap();
        let logs_only = full_pubdata.with_pubdata_content(PubdataContent::LogsOnly);
        assert_ne!(full_pubdata.hash(), logs_only.hash());
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
        // Field order is [chain_id, fri, max_tx_gas_limit, pubdata_content], one word each on
        // the 64-bit test host; drop max_tx_gas_limit (index 2) below the floor.
        serialized[2] = (DEFAULT_MAX_TX_GAS_LIMIT - 1) as usize;
        let mut iter = serialized.into_iter();

        let config = ChainConfig::from_iter(&mut iter).expect("deserialization must not validate");
        assert!(config.validate().is_err());
    }
}

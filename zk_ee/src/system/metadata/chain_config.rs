use crate::common_structs::da_commitment_scheme::PubdataContent;
use crate::internal_error;
use crate::oracle::memory_io::host::{write_padded_image, WriteQueryOutput};
use crate::oracle::memory_io::{
    normalize_bool, ContinuousDeserializable, MemoryOracle, OracleQuery,
};
use crate::oracle::query_ids::CHAIN_CONFIG_QUERY_ID;
use crate::system::errors::internal::InternalError;
use alloc::vec::Vec;
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
///
/// `#[repr(C)]`: the oracle writes it memcpy-like (see the `ContinuousDeserializable` impl).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
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
    pub fn read_from_oracle(oracle: &mut impl MemoryOracle) -> Result<Self, InternalError> {
        ChainConfigQuery::get(oracle, ())
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
        *hasher.finalize()
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

// The same layout on every target: `u64` is aligned to 8 on the proving target as well.
const _: () = {
    assert!(core::mem::size_of::<ChainConfig>() == 32);
    assert!(core::mem::align_of::<ChainConfig>() == 8);
    assert!(core::mem::offset_of!(ChainConfig, chain_id) == 0);
    assert!(core::mem::offset_of!(ChainConfig, fri_proof_verification_enabled) == 8);
    assert!(core::mem::offset_of!(ChainConfig, max_tx_gas_limit) == 16);
    assert!(core::mem::offset_of!(ChainConfig, pubdata_content) == 24);
    assert!(core::mem::size_of::<PubdataContent>() == 1);
};

// The flag is normalized the way a short `bool` is decoded (any non-zero byte is `true`), the pubdata
// content is checked to be a valid ID of the `#[repr(u8)]` enum before it is read as such, and the padding
// is ignored. As for the iterator-based protocol, this is a pure parse: the chain-level limitations are
// checked by `ChainConfig::validate`.
// SAFETY: see above and the layout assertions
unsafe impl ContinuousDeserializable for ChainConfig {
    #[inline(always)]
    unsafe fn validate<'a>(this: *mut Self) -> Result<&'a mut Self, InternalError> {
        // SAFETY: the fields are inside the value, and initialized as per the caller contract; the pubdata
        // content is read as its byte, and only as the enum once checked
        unsafe {
            normalize_bool(&raw mut (*this).fri_proof_verification_enabled);
            let pubdata_content = (&raw const (*this).pubdata_content).cast::<u8>().read();
            PubdataContent::try_from(pubdata_content)
                .map_err(|_| internal_error!("Invalid pubdata content"))?;
            Ok(&mut *this)
        }
    }
}

/// The fields and zeroed padding: the image the querier validates.
impl WriteQueryOutput for ChainConfig {
    fn write_output(&self, response: &mut Vec<u32>) {
        write_padded_image(response, |image: *mut Self| {
            // SAFETY: the image is a valid, aligned place for `Self`
            unsafe {
                (&raw mut (*image).chain_id).write(self.chain_id);
                (&raw mut (*image).fri_proof_verification_enabled)
                    .write(self.fri_proof_verification_enabled);
                (&raw mut (*image).max_tx_gas_limit).write(self.max_tx_gas_limit);
                (&raw mut (*image).pubdata_content).write(self.pubdata_content);
            }
        });
    }
}

pub struct ChainConfigQuery;

impl OracleQuery for ChainConfigQuery {
    const QUERY_ID: u32 = CHAIN_CONFIG_QUERY_ID;
    type Input = ();
    type Output = ChainConfig;
}

impl ChainConfigMetadata for ChainConfig {
    fn chain_config(&self) -> ChainConfig {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The response words of the chain config query, as the oracle writes them.
    fn response(config: &ChainConfig) -> Vec<u32> {
        let mut response = Vec::new();
        config.write_output(&mut response);
        response
    }

    /// The chain config the querier reads from `response`.
    fn read(response: Vec<u32>) -> Result<ChainConfig, InternalError> {
        use crate::oracle::memory_io::host::{InProcessMemoryOracle, QuerierMemory};
        let mut response = Some(response);
        let mut oracle = InProcessMemoryOracle::new(move |query_id, _, _: &dyn QuerierMemory| {
            assert_eq!(query_id, CHAIN_CONFIG_QUERY_ID);
            response.take().unwrap()
        });
        ChainConfig::read_from_oracle(&mut oracle)
    }

    #[test]
    fn chain_config_roundtrips_through_the_oracle() {
        let original = ChainConfig::new(37, true, DEFAULT_MAX_TX_GAS_LIMIT).unwrap();
        let response = response(&original);
        assert_eq!(response.len(), 8);
        assert_eq!(read(response).unwrap(), original);
    }

    #[test]
    fn chain_config_from_the_oracle_is_validated() {
        let original = ChainConfig::default_for_chain();
        // the flag is the byte at offset 8, the pubdata content the byte at offset 24
        let mut flag_set = response(&original);
        flag_set[2] = 0xff;
        assert!(read(flag_set).unwrap().fri_proof_verification_enabled());
        let mut invalid_pubdata_content = response(&original);
        invalid_pubdata_content[6] = 2;
        assert!(read(invalid_pubdata_content).is_err());
        // padding is ignored
        let mut padding_set = response(&original);
        padding_set[3] = 0xffff_ffff;
        padding_set[7] = 0xffff_ff00;
        assert_eq!(read(padding_set).unwrap(), original);
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
        assert_eq!(read(response(&config)).unwrap(), config);
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
    fn reading_from_the_oracle_does_not_validate() {
        // Validation is enforced at the system boundary, not when the config is
        // received, so a below-floor value is read successfully and is only
        // rejected by an explicit `validate()`.
        let mut response = response(&ChainConfig::default_for_chain());
        // `max_tx_gas_limit` is at offset 16: words 4 (low) and 5 (high)
        response[4] = (DEFAULT_MAX_TX_GAS_LIMIT - 1) as u32;
        response[5] = 0;

        let config = read(response).expect("reading must not validate");
        assert!(config.validate().is_err());
    }
}

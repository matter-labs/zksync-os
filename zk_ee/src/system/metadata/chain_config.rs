use crate::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use crate::system::errors::internal::InternalError;
use crate::{internal_error, utils::exact_size_chain::ExactSizeChain};

/// Current canonical version of the chain configuration encoded in public input.
pub const SUPPORTED_CHAIN_CONFIG_VERSION: u32 = 1;

/// Default EIP-170 deployed contract code-size limit.
pub const DEFAULT_MAX_CONTRACT_SIZE: u32 = 0x6000;

/// A limit that can be switched off at the chain configuration level.
///
/// Disabled limits are canonicalized as `(enabled = false, value = 0)`.
/// This prevents two semantically equivalent disabled limits from producing
/// different public-input preimages.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigurableLimitU32 {
    enabled: bool,
    value: u32,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ConfigurableLimitU32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct RawConfigurableLimitU32 {
            enabled: bool,
            value: u32,
        }

        let raw = <RawConfigurableLimitU32 as serde::Deserialize>::deserialize(deserializer)?;
        let limit = Self {
            enabled: raw.enabled,
            value: raw.value,
        };
        limit
            .validate()
            .map_err(|_| serde::de::Error::custom("invalid configurable limit"))?;

        Ok(limit)
    }
}

impl ConfigurableLimitU32 {
    /// Constructs a trusted in-code limit and canonicalizes disabled values to
    /// zero. Untrusted input must use deserialization, which rejects
    /// non-canonical disabled values.
    pub const fn new(enabled: bool, value: u32) -> Self {
        if enabled {
            Self { enabled, value }
        } else {
            Self {
                enabled: false,
                value: 0,
            }
        }
    }

    pub const fn enabled(value: u32) -> Self {
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

    pub const fn value(&self) -> u32 {
        self.value
    }

    pub fn validate(&self) -> Result<(), InternalError> {
        if !self.enabled && self.value != 0 {
            return Err(internal_error!(
                "disabled configurable limit must have zero value"
            ));
        }

        Ok(())
    }

    pub fn is_satisfied_by(&self, value: u32) -> bool {
        !self.enabled || value <= self.value
    }
}

impl UsizeSerializable for ConfigurableLimitU32 {
    const USIZE_LEN: usize =
        <bool as UsizeSerializable>::USIZE_LEN + <u32 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.enabled),
            UsizeSerializable::iter(&self.value),
        )
    }
}

impl UsizeDeserializable for ConfigurableLimitU32 {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let enabled = UsizeDeserializable::from_iter(src)?;
        let value = UsizeDeserializable::from_iter(src)?;
        let limit = Self { enabled, value };
        limit.validate()?;

        Ok(limit)
    }
}

/// Static chain-level execution rules committed into the batch public input.
///
/// The bootloader validates the version before execution. Changing this value
/// is a protocol-upgrade boundary and batches must not span configurations.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainConfig {
    version: u32,
    fri_proof_verification_enabled: bool,
    /// Optional deployed bytecode size limit. This does not configure the
    /// separate EIP-3860 initcode-size limit.
    max_contract_size: ConfigurableLimitU32,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ChainConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct RawChainConfig {
            version: u32,
            fri_proof_verification_enabled: bool,
            max_contract_size: ConfigurableLimitU32,
        }

        let raw = <RawChainConfig as serde::Deserialize>::deserialize(deserializer)?;
        let config = Self {
            version: raw.version,
            fri_proof_verification_enabled: raw.fri_proof_verification_enabled,
            max_contract_size: raw.max_contract_size,
        };
        config
            .validate()
            .map_err(|_| serde::de::Error::custom("invalid chain config"))?;

        Ok(config)
    }
}

impl ChainConfig {
    pub fn new(
        version: u32,
        fri_proof_verification_enabled: bool,
        max_contract_size: ConfigurableLimitU32,
    ) -> Result<Self, InternalError> {
        let config = Self {
            version,
            fri_proof_verification_enabled,
            max_contract_size,
        };
        config.validate()?;

        Ok(config)
    }

    pub const fn default_for_chain() -> Self {
        Self {
            version: SUPPORTED_CHAIN_CONFIG_VERSION,
            fri_proof_verification_enabled: false,
            max_contract_size: ConfigurableLimitU32::enabled(DEFAULT_MAX_CONTRACT_SIZE),
        }
    }

    pub const fn with_fri_proof_verification_enabled(mut self, enabled: bool) -> Self {
        self.fri_proof_verification_enabled = enabled;
        self
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub const fn fri_proof_verification_enabled(&self) -> bool {
        self.fri_proof_verification_enabled
    }

    pub const fn max_contract_size(&self) -> ConfigurableLimitU32 {
        self.max_contract_size
    }

    pub fn validate(&self) -> Result<(), InternalError> {
        if self.version != SUPPORTED_CHAIN_CONFIG_VERSION {
            return Err(internal_error!("unsupported chain config version"));
        }
        self.max_contract_size.validate()
    }
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self::default_for_chain()
    }
}

impl UsizeSerializable for ChainConfig {
    const USIZE_LEN: usize = <u32 as UsizeSerializable>::USIZE_LEN
        + <bool as UsizeSerializable>::USIZE_LEN
        + <ConfigurableLimitU32 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.version),
                UsizeSerializable::iter(&self.fri_proof_verification_enabled),
            ),
            UsizeSerializable::iter(&self.max_contract_size),
        )
    }
}

impl UsizeDeserializable for ChainConfig {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let version = UsizeDeserializable::from_iter(src)?;
        let fri_proof_verification_enabled = UsizeDeserializable::from_iter(src)?;
        let max_contract_size = UsizeDeserializable::from_iter(src)?;

        let config = Self {
            version,
            fri_proof_verification_enabled,
            max_contract_size,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_limit_must_be_canonical() {
        assert_eq!(ConfigurableLimitU32::new(false, 1).value(), 0);
        assert_eq!(ConfigurableLimitU32::disabled().value(), 0);
    }

    #[test]
    fn chain_config_rejects_unknown_version() {
        let invalid = ChainConfig {
            version: SUPPORTED_CHAIN_CONFIG_VERSION + 1,
            fri_proof_verification_enabled: false,
            max_contract_size: ConfigurableLimitU32::enabled(DEFAULT_MAX_CONTRACT_SIZE),
        };

        assert!(invalid.validate().is_err());
        assert!(ChainConfig::new(
            SUPPORTED_CHAIN_CONFIG_VERSION + 1,
            false,
            ConfigurableLimitU32::enabled(DEFAULT_MAX_CONTRACT_SIZE),
        )
        .is_err());
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
    fn chain_config_usize_deserialization_rejects_unknown_version() {
        let mut serialized = ChainConfig::default_for_chain().iter().collect::<Vec<_>>();
        serialized[0] = SUPPORTED_CHAIN_CONFIG_VERSION as usize + 1;
        let mut iter = serialized.into_iter();

        assert!(ChainConfig::from_iter(&mut iter).is_err());
    }

    #[test]
    fn disabled_limit_deserialization_rejects_non_canonical_value() {
        let mut serialized = vec![false as usize, 10usize].into_iter();

        assert!(ConfigurableLimitU32::from_iter(&mut serialized).is_err());
    }
}
  basic_bootloader/src/bootloader/block_flow/ethereum/metadata_op.rs  basic_bootloader/src/bootloader/block_flow/metadata_init_op.rs  basic_bootloader/src/bootloader/block_flow/zk/batch_data.rs  basic_bootloader/src/bootloader/block_flow/zk/metadata_op.rs  basic_bootloader/src/bootloader/block_flow/zk/post_init_op.rs  basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/post_tx_op_proving_multiblock_batch.rs  basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/post_tx_op_proving_singleblock_batch.rs  basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/public_input.rs  basic_bootloader/src/bootloader/config.rs  basic_bootloader/src/bootloader/mod.rs  basic_bootloader/src/bootloader/transaction_flow/zk/fri.rs  basic_bootloader/src/bootloader/transaction_flow/zk/validation_impl.rs  evm_interpreter/src/interpreter.rs  evm_interpreter/src/lib.rs  forward_system/src/run/convert.rs  forward_system/src/run/interface_impl.rs  forward_system/src/run/mod.rs  forward_system/src/run/query_processors/mod.rs  forward_system/src/system/bootloader.rs  proof_running_system/src/system/bootloader.rs  system_hooks/src/addresses_constants.rs  system_hooks/src/call_hooks/contract_deployer_temp.rs  system_hooks/src/call_hooks/set_bytecode_on_address.rs  tests/block_reexecutor/src/rpc_client.rs  tests/block_reexecutor/src/rpc_oracle.rs  tests/evm_divergence_validator/src/runner.rs  tests/evm_tester/src/vm/zk_ee/mod.rs  tests/instances/eth_runner/src/block.rs  tests/instances/system_hooks/src/lib.rs  tests/instances/unit/src/initial_slot_regression.rs  tests/instances/unit/src/malicious_oracle.rs  tests/rig/src/chain.rs  tests/rig/src/lib.rs  tests/rig/src/revm_consistency_checker.rs  zk_ee/src/oracle/query_ids.rs  zk_ee/src/system/metadata/basic_metadata.rs  zk_ee/src/system/metadata/mod.rs  zk_ee/src/system/metadata/system_metadata.rs  zk_ee/src/system/metadata/zk_metadata.rs  zk_ee/src/system/mod.rs
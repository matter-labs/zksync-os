use crate::system_implementation::flat_storage_model::{
    BytecodeAndAccountDataPreimagesStorage, PreimageRequest,
};
use alloc::alloc::Allocator;
use crypto::MiniDigest;
use ruint::aliases::U256;
use storage_models::common_structs::PreimageCacheModel;
use zk_ee::common_structs::{PreimageType, ValueDiffCompressionStrategy};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::{InternalError, SystemError};
use zk_ee::system::{IOResultKeeper, Resources};
use zk_ee::system_io_oracle::IOOracle;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Default, PartialOrd, Ord, Hash)]
///
/// Stores multiple account version information packed in u64.
/// Holds information about(7th is the most signifact byte):
/// - deployment status (u8, 7th byte)
/// - EE version/type (EVM, EraVM, etc.) (u8, 6th byte)
/// - code version (u8) - ee specific (currently both EVM and IWASM use 1, 5th byte)
/// - system aux bitmask (u8, 4th byte)
/// - EE aux bitmask (u8, 3th byte)
/// - 3 less signifact(0-2) bytes currently set to 0, may be used in the future.
///
pub struct VersioningData<const DEPLOYED: u8>(u64);

impl<const DEPLOYED: u8> VersioningData<DEPLOYED> {
    pub const fn empty_deployed() -> Self {
        Self((DEPLOYED as u64) << 56)
    }

    pub const fn empty_non_deployed() -> Self {
        Self(0u64)
    }

    pub const fn is_deployed(&self) -> bool {
        (self.0 >> 56) as u8 == DEPLOYED
    }

    pub fn set_as_deployed(&mut self) {
        self.0 = self.0 & 0x00ffffff_ffffffff | ((DEPLOYED as u64) << 56)
    }

    pub const fn ee_version(&self) -> u8 {
        (self.0 >> 48) as u8
    }

    pub fn set_ee_version(&mut self, value: u8) {
        self.0 = self.0 & 0xff00ffff_ffffffff | ((value as u64) << 48)
    }

    pub const fn code_version(&self) -> u8 {
        (self.0 >> 40) as u8
    }

    pub fn set_code_version(&mut self, value: u8) {
        self.0 = self.0 & 0xffff00ff_ffffffff | ((value as u64) << 40)
    }

    pub const fn system_aux_bitmask(&self) -> u8 {
        (self.0 >> 32) as u8
    }

    pub fn set_system_aux_bitmask(&mut self, value: u8) {
        self.0 = self.0 & 0xffffff00_ffffffff | ((value as u64) << 32)
    }

    pub const fn ee_aux_bitmask(&self) -> u8 {
        (self.0 >> 24) as u8
    }

    pub fn set_ee_aux_bitmask(&mut self, value: u8) {
        self.0 = self.0 & 0xffffffff_00ffffff | ((value as u64) << 24)
    }

    pub fn from_u64(value: u64) -> Self {
        Self(value)
    }

    pub fn into_u64(self) -> u64 {
        self.0
    }
}

impl<const N: u8> core::fmt::Debug for VersioningData<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x{:016x}", self.0)
    }
}

pub const DEFAULT_ADDRESS_SPECIFIC_IMMUTABLE_DATA_VERSION: u8 = 1;

#[derive(Default, Clone)]
pub struct AccountPropertiesMetadata {
    pub deployed_in_tx: u32,
    /// Transaction where this account was last accessed.
    /// Considered warm if equal to Some(current_tx)
    pub last_touched_in_tx: Option<u32>,
}

impl AccountPropertiesMetadata {
    pub fn considered_warm(&self, current_tx_number: u32) -> bool {
        self.last_touched_in_tx == Some(current_tx_number)
    }
}

///
/// Encoding layout:
/// versioningData:               u64, BE @ [0..8] (see above)
/// nonce:                        u64, BE @ [8..16]
/// balance:                     U256, BE @ [16..48]
/// bytecode_hash:            Bytes32,    @ [48..80]
/// bytecode_len:                 u32, BE @ [80..84]
/// artifacts_len:                u32, BE @ [84..88]
/// observable_bytecode_hash: Bytes32,    @ [88..120]
/// observable_bytecode_len:      u32, BE @ [120..124]
///
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct AccountProperties {
    pub versioning_data: VersioningData<DEFAULT_ADDRESS_SPECIFIC_IMMUTABLE_DATA_VERSION>,
    pub nonce: u64,
    pub balance: U256,
    pub bytecode_hash: Bytes32,
    pub bytecode_len: u32,
    pub artifacts_len: u32,
    pub observable_bytecode_hash: Bytes32,
    pub observable_bytecode_len: u32,
}

impl AccountProperties {
    pub const TRIVIAL_VALUE: Self = Self {
        versioning_data: VersioningData::empty_non_deployed(),
        nonce: 0,
        balance: U256::ZERO,
        bytecode_hash: Bytes32::ZERO,
        bytecode_len: 0,
        artifacts_len: 0,
        observable_bytecode_hash: Bytes32::ZERO,
        observable_bytecode_len: 0,
    };
}

impl Default for AccountProperties {
    fn default() -> Self {
        Self::TRIVIAL_VALUE
    }
}

impl AccountProperties {
    pub const ENCODED_SIZE: usize = 124;

    pub fn encoding(&self) -> [u8; Self::ENCODED_SIZE] {
        let mut buffer = [0u8; Self::ENCODED_SIZE];
        buffer[0..8].copy_from_slice(&self.versioning_data.into_u64().to_be_bytes());
        buffer[8..16].copy_from_slice(&self.nonce.to_be_bytes());
        buffer[16..48].copy_from_slice(&self.balance.to_be_bytes::<32>());
        buffer[48..80].copy_from_slice(self.bytecode_hash.as_u8_ref());
        buffer[80..84].copy_from_slice(&self.bytecode_len.to_be_bytes());
        buffer[84..88].copy_from_slice(&self.artifacts_len.to_be_bytes());
        buffer[88..120].copy_from_slice(self.observable_bytecode_hash.as_u8_ref());
        buffer[120..124].copy_from_slice(&self.observable_bytecode_len.to_be_bytes());
        buffer
    }

    pub fn decode(input: &[u8; Self::ENCODED_SIZE]) -> Self {
        Self {
            versioning_data: VersioningData::from_u64(u64::from_be_bytes(
                <&[u8] as TryInto<[u8; 8]>>::try_into(&input[0..8]).unwrap(),
            )),
            nonce: u64::from_be_bytes(input[8..16].try_into().unwrap()),
            balance: U256::from_be_slice(&input[16..48]),
            bytecode_hash: Bytes32::from(
                <&[u8] as TryInto<[u8; 32]>>::try_into(&input[48..80]).unwrap(),
            ),
            bytecode_len: u32::from_be_bytes(input[80..84].try_into().unwrap()),
            artifacts_len: u32::from_be_bytes(input[84..88].try_into().unwrap()),
            observable_bytecode_hash: Bytes32::from(
                <&[u8] as TryInto<[u8; 32]>>::try_into(&input[88..120]).unwrap(),
            ),
            observable_bytecode_len: u32::from_le_bytes(input[120..124].try_into().unwrap()),
        }
    }

    pub fn compute_hash(&self) -> Bytes32 {
        use crypto::blake2s::Blake2s256;
        use crypto::MiniDigest;
        // efficient hashing without copying
        let mut hasher = Blake2s256::new();
        hasher.update(self.versioning_data.into_u64().to_be_bytes());
        hasher.update(self.nonce.to_be_bytes());
        hasher.update(self.balance.to_be_bytes::<32>());
        hasher.update(self.bytecode_hash.as_u8_ref());
        hasher.update(self.bytecode_len.to_be_bytes());
        hasher.update(self.artifacts_len.to_be_bytes());
        hasher.update(self.observable_bytecode_hash.as_u8_ref());
        hasher.update(self.observable_bytecode_len.to_be_bytes());
        hasher.finalize().into()
    }

    pub fn diff_compression_length(initial: &Self, r#final: &Self) -> Result<u32, InternalError> {
        match (
            initial.versioning_data.is_deployed(),
            r#final.versioning_data.is_deployed(),
        ) {
            (true, false) => Err(InternalError(
                "Account destructed at the end of the tx/block",
            )),
            (false, true) => {
                Ok(
                    1u32 // metadata byte
                    + 8 // versioning data
                    + ValueDiffCompressionStrategy::optimal_compression_length_u256(initial.nonce.try_into().map_err(|_| InternalError("u64 into U256"))?, r#final.nonce.try_into().map_err(|_| InternalError("u64 into U256"))?) as u32 // nonce diff
                    + ValueDiffCompressionStrategy::optimal_compression_length_u256(initial.balance, r#final.balance) as u32 // balance diff
                    + 4 // bytecode len
                    + r#final.bytecode_len // bytecode
                    + 4 // artifacts len
                    + 4, // observable bytecode len
                )
            }
            (_, _) => {
                // if deployment status didn't change, only balance and nonce can be changed
                debug_assert_eq!(initial.versioning_data, r#final.versioning_data);
                debug_assert_eq!(initial.bytecode_hash, r#final.bytecode_hash);
                debug_assert_eq!(
                    initial.observable_bytecode_hash,
                    r#final.observable_bytecode_hash
                );
                debug_assert_eq!(initial.bytecode_len, r#final.bytecode_len);
                debug_assert_eq!(
                    initial.observable_bytecode_len,
                    r#final.observable_bytecode_len
                );
                debug_assert_eq!(initial.artifacts_len, r#final.artifacts_len);

                if initial.nonce == r#final.nonce && initial.balance == r#final.balance {
                    return Ok(0);
                }
                let mut length = 1u32; // metadata byte
                if initial.nonce != r#final.nonce {
                    length += ValueDiffCompressionStrategy::optimal_compression_length_u256(
                        initial
                            .nonce
                            .try_into()
                            .map_err(|_| InternalError("u64 into U256"))?,
                        r#final
                            .nonce
                            .try_into()
                            .map_err(|_| InternalError("u64 into U256"))?,
                    ) as u32; // nonce diff
                }
                if initial.balance != r#final.balance {
                    length += ValueDiffCompressionStrategy::optimal_compression_length_u256(
                        initial.balance,
                        r#final.balance,
                    ) as u32; // balance diff
                }
                Ok(length)
            }
        }
    }

    pub fn diff_compression<const PROOF_ENV: bool, R: Resources, A: Allocator + Clone>(
        initial: &Self,
        r#final: &Self,
        hasher: &mut impl MiniDigest,
        result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<(), InternalError> {
        match (
            initial.versioning_data.is_deployed(),
            r#final.versioning_data.is_deployed(),
        ) {
            (true, false) => Err(InternalError(
                "Account destructed at the end of the tx/block",
            )),
            (false, true) => {
                let metadata_byte = 4u8;
                hasher.update([metadata_byte]);
                ValueDiffCompressionStrategy::optimal_compression_u256(
                    initial
                        .nonce
                        .try_into()
                        .map_err(|_| InternalError("u64 into U256"))?,
                    r#final
                        .nonce
                        .try_into()
                        .map_err(|_| InternalError("u64 into U256"))?,
                    hasher,
                    result_keeper,
                );
                ValueDiffCompressionStrategy::optimal_compression_u256(
                    initial.balance,
                    r#final.balance,
                    hasher,
                    result_keeper,
                );
                hasher.update(r#final.bytecode_len.to_be_bytes());
                let preimage_type = PreimageRequest {
                    hash: r#final.bytecode_hash,
                    expected_preimage_len_in_bytes: r#final.bytecode_len,
                    preimage_type: PreimageType::Bytecode,
                };
                let mut resources = R::FORMAL_INFINITE;
                let bytecode = preimages_cache
                    .get_preimage::<PROOF_ENV>(
                        ExecutionEnvironmentType::NoEE,
                        &preimage_type,
                        &mut resources,
                        oracle,
                    )
                    .map_err(|err| match err {
                        SystemError::OutOfErgs => InternalError("Out of ergs on infinite ergs"),
                        SystemError::OutOfNativeResources => {
                            InternalError("Out of native on infinite")
                        }
                        SystemError::Internal(i) => i,
                    })?;
                hasher.update(bytecode);
                hasher.update(r#final.artifacts_len.to_be_bytes());
                hasher.update(r#final.observable_bytecode_len.to_be_bytes());
                Ok(())
            }
            (_, _) => {
                // if deployment status didn't change, only balance and nonce can be changed
                debug_assert_eq!(initial.versioning_data, r#final.versioning_data);
                debug_assert_eq!(initial.bytecode_hash, r#final.bytecode_hash);
                debug_assert_eq!(
                    initial.observable_bytecode_hash,
                    r#final.observable_bytecode_hash
                );
                debug_assert_eq!(initial.bytecode_len, r#final.bytecode_len);
                debug_assert_eq!(
                    initial.observable_bytecode_len,
                    r#final.observable_bytecode_len
                );
                debug_assert_eq!(initial.artifacts_len, r#final.artifacts_len);

                if initial.nonce == r#final.nonce && initial.balance == r#final.balance {
                    return Ok(());
                }
                let mut metadata_byte = 4u8;
                if initial.nonce != r#final.nonce {
                    metadata_byte |= 1 << 3;
                }
                if initial.balance != r#final.balance {
                    metadata_byte |= 2 << 3;
                }
                hasher.update([metadata_byte]);
                if initial.nonce != r#final.nonce {
                    ValueDiffCompressionStrategy::optimal_compression_u256(
                        initial
                            .nonce
                            .try_into()
                            .map_err(|_| InternalError("u64 into U256"))?,
                        r#final
                            .nonce
                            .try_into()
                            .map_err(|_| InternalError("u64 into U256"))?,
                        hasher,
                        result_keeper,
                    );
                }
                if initial.balance != r#final.balance {
                    ValueDiffCompressionStrategy::optimal_compression_u256(
                        initial.balance,
                        r#final.balance,
                        hasher,
                        result_keeper,
                    );
                }
                Ok(())
            }
        }
    }
}

use crate::system_implementation::flat_storage_model::{
    BytecodeAndAccountDataPreimagesStorage, PreimageRequest,
};
use alloc::alloc::Allocator;
use crypto::MiniDigest;
use ruint::aliases::U256;
use storage_models::common_structs::PreimageCacheModel;
use zk_ee::common_structs::{PreimageType, ValueDiffCompressionStrategy};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::system::errors::{InternalError, SystemError};
use zk_ee::system::{IOResultKeeper, Resources};
use zk_ee::system_io_oracle::IOOracle;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Default, PartialOrd, Ord, Hash)]
///
/// Stores multiple account version information packed in u64.
/// Holds information about(7th is the most significant byte):
/// - deployment status (u8, 7th byte)
/// - EE version/type (EVM, EraVM, etc.) (u8, 6th byte)
/// - code version (u8) - ee specific (currently both EVM and IWASM use 1, 5th byte)
/// - system aux bitmask (u8, 4th byte)
/// - EE aux bitmask (u8, 3th byte)
/// - 3 less significant(0-2) bytes currently set to 0, may be used in the future.
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
    /// None if the account hasn't been deployed in the current block.
    pub deployed_in_tx: Option<u32>,
    /// Transaction where this account was last accessed.
    /// Considered warm if equal to Some(current_tx)
    pub last_touched_in_tx: Option<u32>,
    /// Special flag that allows to avoid publishing bytecode for deployed account.
    /// In practice, it can be set to `true` only during special protocol upgrade txs.
    /// For protocol upgrades it's ensured by governance that bytecodes are already published separately.
    pub not_publish_bytecode: bool,
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
/// unpadded_code_len:                 u32, BE @ [80..84]
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
    pub unpadded_code_len: u32,
    pub artifacts_len: u32,
    pub observable_bytecode_hash: Bytes32,
    // TODO(EVM-1116): document the need for observable_bytecode_len
    pub observable_bytecode_len: u32,
}

#[inline(always)]
pub const fn bytecode_padding_len(deployed_len: usize) -> usize {
    let word = evm_interpreter::BYTECODE_ALIGNMENT;
    let rem = deployed_len % word;
    if rem == 0 {
        0
    } else {
        word - rem
    }
}

impl AccountProperties {
    pub const TRIVIAL_VALUE: Self = Self {
        versioning_data: VersioningData::empty_non_deployed(),
        nonce: 0,
        balance: U256::ZERO,
        bytecode_hash: Bytes32::ZERO,
        unpadded_code_len: 0,
        artifacts_len: 0,
        observable_bytecode_hash: Bytes32::ZERO,
        observable_bytecode_len: 0,
    };

    pub fn full_bytecode_len(&self) -> u32 {
        let padding = bytecode_padding_len(self.unpadded_code_len as usize);
        self.unpadded_code_len + (padding as u32) + self.artifacts_len
    }
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
        buffer[80..84].copy_from_slice(&self.unpadded_code_len.to_be_bytes());
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
            unpadded_code_len: u32::from_be_bytes(input[80..84].try_into().unwrap()),
            artifacts_len: u32::from_be_bytes(input[84..88].try_into().unwrap()),
            observable_bytecode_hash: Bytes32::from(
                <&[u8] as TryInto<[u8; 32]>>::try_into(&input[88..120]).unwrap(),
            ),
            observable_bytecode_len: u32::from_be_bytes(input[120..124].try_into().unwrap()),
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
        hasher.update(self.unpadded_code_len.to_be_bytes());
        hasher.update(self.artifacts_len.to_be_bytes());
        hasher.update(self.observable_bytecode_hash.as_u8_ref());
        hasher.update(self.observable_bytecode_len.to_be_bytes());
        hasher.finalize().into()
    }

    ///
    /// Estimate account properties diff compression length.
    /// For more details about compression, see the `diff_compression` method(below).
    ///
    pub fn diff_compression_length(
        initial: &Self,
        r#final: &Self,
        not_publish_bytecode: bool,
    ) -> Result<u32, InternalError> {
        // if something except nonce and balance changed, we'll encode full diff, for all the fields
        let full_diff = initial.versioning_data != r#final.versioning_data
            || initial.bytecode_hash != r#final.bytecode_hash
            || initial.unpadded_code_len != r#final.unpadded_code_len
            || initial.artifacts_len != r#final.artifacts_len
            || initial.observable_bytecode_len != r#final.observable_bytecode_len
            || initial.observable_bytecode_hash != r#final.observable_bytecode_hash;
        if full_diff {
            Ok(if not_publish_bytecode {
                1u32 // metadata byte
                    + 8 // versioning data
                    + ValueDiffCompressionStrategy::optimal_compression_length_u256(initial.nonce.try_into().map_err(|_| internal_error!("u64 into U256"))?, r#final.nonce.try_into().map_err(|_| internal_error!("u64 into U256"))?) as u32 // nonce diff
                    + ValueDiffCompressionStrategy::optimal_compression_length_u256(initial.balance, r#final.balance) as u32 // balance diff
                    + 32 // bytecode hash
                    + 4 // artifacts len
                    + 4 // observable bytecode len
            } else {
                1u32 // metadata byte
                    + 8 // versioning data
                    + ValueDiffCompressionStrategy::optimal_compression_length_u256(initial.nonce.try_into().map_err(|_| internal_error!("u64 into U256"))?, r#final.nonce.try_into().map_err(|_| internal_error!("u64 into U256"))?) as u32 // nonce diff
                    + ValueDiffCompressionStrategy::optimal_compression_length_u256(initial.balance, r#final.balance) as u32 // balance diff
                    + 4 // unpadded code len
                    + 4 // artifacts len
                    + r#final.full_bytecode_len() // bytecode
                    + 4 // observable bytecode len
            })
        } else {
            if initial.nonce == r#final.nonce && initial.balance == r#final.balance {
                return Err(internal_error!(
                    "Account properties diff compression shouldn't be called for same values",
                ));
            }
            let mut length = 1u32; // metadata byte
            if initial.nonce != r#final.nonce {
                length += ValueDiffCompressionStrategy::optimal_compression_length_u256(
                    initial
                        .nonce
                        .try_into()
                        .map_err(|_| internal_error!("u64 into U256"))?,
                    r#final
                        .nonce
                        .try_into()
                        .map_err(|_| internal_error!("u64 into U256"))?,
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

    ///
    /// Compress account properties diff.
    /// The diffs for accounts will be encoded together with state diffs under corresponding storage keys.
    /// So, in fact, this compression is an "extension" for storage value compression.
    /// For storage value we have one metadata byte and use 3 less significant bits to describe compression type.
    /// 4(0-3) types are used for values, so we'll use 4 as the account diff compression type.
    /// 5 most significant bits of metadata byte can be used to save additional info for encoding type.
    ///
    /// For account data we have following encoding formats(index encoded in the 5 most significant bits of the metadata byte, 3 less significant == 4):
    /// 0(full data): `versioning_data(8 BE bytes) & nonce_diff(using storage value strategy)
    /// & balance_diff & unpadded_code_len(4 BE bytes) &  artifacts_len (4 BE bytes) &
    /// & bytecode & observable_len (4 BE bytes)`
    /// 1: `nonce_diff (using storage value strategy)`
    /// 2: `balance_diff (using storage value strategy)`
    /// 3: `nonce_diff (using storage value strategy) & balance_diff (using storage value strategy)`
    /// 4. `versioning_data(8 BE bytes) & nonce_diff(using storage value strategy) & balance_diff & bytecode_hash (32 bytes) & artifacts_len (4 BE bytes) & observable_len (4 BE bytes)`
    ///
    /// The last format(4) created for force deployments during protocol upgrades. We publish only bytecode hash, but it's guaranteed by the governance that bytecode will be published separately.
    ///
    pub fn diff_compression<const PROOF_ENV: bool, R: Resources, A: Allocator + Clone>(
        initial: &Self,
        r#final: &Self,
        not_publish_bytecode: bool,
        hasher: &mut impl MiniDigest,
        result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
        preimages_cache: &mut BytecodeAndAccountDataPreimagesStorage<R, A>,
        oracle: &mut impl IOOracle,
    ) -> Result<(), InternalError> {
        // if something except nonce and balance changed, we'll encode full diff, for all the fields
        let full_diff = initial.versioning_data != r#final.versioning_data
            || initial.bytecode_hash != r#final.bytecode_hash
            || initial.unpadded_code_len != r#final.unpadded_code_len
            || initial.artifacts_len != r#final.artifacts_len
            || initial.observable_bytecode_len != r#final.observable_bytecode_len
            || initial.observable_bytecode_hash != r#final.observable_bytecode_hash;

        if full_diff {
            // Account encoding (0b100), option 0 (0b000100) or option 4 (0b100100), see function specs.
            let metadata_byte = if not_publish_bytecode {
                0b00100100
            } else {
                0b00000100
            };

            hasher.update([metadata_byte]);
            result_keeper.pubdata(&[metadata_byte]);
            hasher.update(r#final.versioning_data.into_u64().to_be_bytes());
            result_keeper.pubdata(&r#final.versioning_data.into_u64().to_be_bytes());
            ValueDiffCompressionStrategy::optimal_compression_u256(
                initial
                    .nonce
                    .try_into()
                    .map_err(|_| internal_error!("u64 into U256"))?,
                r#final
                    .nonce
                    .try_into()
                    .map_err(|_| internal_error!("u64 into U256"))?,
                hasher,
                result_keeper,
            );
            ValueDiffCompressionStrategy::optimal_compression_u256(
                initial.balance,
                r#final.balance,
                hasher,
                result_keeper,
            );

            if not_publish_bytecode {
                hasher.update(r#final.bytecode_hash.as_u8_ref());
                result_keeper.pubdata(r#final.bytecode_hash.as_u8_ref());
            } else {
                hasher.update(r#final.unpadded_code_len.to_be_bytes());
                result_keeper.pubdata(&r#final.unpadded_code_len.to_be_bytes());
                hasher.update(r#final.artifacts_len.to_be_bytes());
                result_keeper.pubdata(&r#final.artifacts_len.to_be_bytes());
                let preimage_type = PreimageRequest {
                    hash: r#final.bytecode_hash,
                    expected_preimage_len_in_bytes: r#final.full_bytecode_len(),
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
                        SystemError::OutOfErgs(_) => {
                            internal_error!("Out of ergs on infinite ergs")
                        }
                        SystemError::OutOfNativeResources(_) => {
                            internal_error!("Out of native on infinite")
                        }
                        SystemError::Internal(i) => i,
                    })?;
                hasher.update(bytecode);
                result_keeper.pubdata(bytecode);
            }
            hasher.update(r#final.observable_bytecode_len.to_be_bytes());
            result_keeper.pubdata(&r#final.observable_bytecode_len.to_be_bytes());
            Ok(())
        } else {
            if initial.nonce == r#final.nonce && initial.balance == r#final.balance {
                return Err(internal_error!(
                    "Account properties diff compression shouldn't be called for same values",
                ));
            }
            let mut metadata_byte = 4u8;
            if initial.nonce != r#final.nonce {
                metadata_byte |= 1 << 3;
            }
            if initial.balance != r#final.balance {
                metadata_byte |= 2 << 3;
            }
            hasher.update([metadata_byte]);
            result_keeper.pubdata(&[metadata_byte]);
            if initial.nonce != r#final.nonce {
                ValueDiffCompressionStrategy::optimal_compression_u256(
                    initial
                        .nonce
                        .try_into()
                        .map_err(|_| internal_error!("u64 into U256"))?,
                    r#final
                        .nonce
                        .try_into()
                        .map_err(|_| internal_error!("u64 into U256"))?,
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

#[cfg(test)]
mod tests {
    use super::AccountProperties;
    use crate::system_implementation::flat_storage_model::{
        BytecodeAndAccountDataPreimagesStorage, PreimageRequest, VersioningData,
    };
    use crypto::blake2s::Blake2s256;
    use crypto::sha3::Keccak256;
    use crypto::MiniDigest;
    use ruint::aliases::U256;
    use std::alloc::Global;
    use storage_models::common_structs::PreimageCacheModel;
    use zk_ee::common_structs::PreimageType;
    use zk_ee::execution_environment_type::ExecutionEnvironmentType;
    use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
    use zk_ee::system::errors::InternalError;
    use zk_ee::system::IOResultKeeper;
    use zk_ee::system::Resource;
    use zk_ee::system_io_oracle::{IOOracle, OracleIteratorTypeMarker};
    use zk_ee::types_config::EthereumIOTypesConfig;
    use zk_ee::utils::*;

    struct TestResultKeeper {
        pub pubdata: Vec<u8>,
    }

    struct TestOracle;

    impl IOOracle for TestOracle {
        type MarkerTiedIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

        fn create_oracle_access_iterator<'a, M: OracleIteratorTypeMarker>(
            &'a mut self,
            _init_value: M::Params,
        ) -> Result<Self::MarkerTiedIterator<'a>, InternalError> {
            unimplemented!()
        }
    }

    impl IOResultKeeper<EthereumIOTypesConfig> for TestResultKeeper {
        fn pubdata<'a>(&mut self, value: &'a [u8]) {
            self.pubdata.extend_from_slice(value)
        }
    }

    #[test]
    fn basic_nonce_change_compression_test() {
        let mut initial = AccountProperties::TRIVIAL_VALUE;
        initial.nonce = 12;

        let mut r#final = AccountProperties::TRIVIAL_VALUE;
        r#final.nonce = 22;

        let optimal_length =
            AccountProperties::diff_compression_length(&initial, &r#final, false).unwrap();

        let mut nop_hasher = NopHasher::new();
        let mut result_keeper = TestResultKeeper { pubdata: vec![] };
        let mut preimages_cache: BytecodeAndAccountDataPreimagesStorage<
            BaseResources<DecreasingNative>,
        > = BytecodeAndAccountDataPreimagesStorage::new_from_parts(Global);
        let mut test_oracle = TestOracle;

        AccountProperties::diff_compression::<false, _, _>(
            &initial,
            &r#final,
            false,
            &mut nop_hasher,
            &mut result_keeper,
            &mut preimages_cache,
            &mut test_oracle,
        )
        .unwrap();
        let compression = result_keeper.pubdata;

        assert_eq!(optimal_length, compression.len() as u32);
        // only nonce changed
        // "Addition" strategy for nonce is optimal in this case
        assert_eq!(compression.len(), 3);
        assert_eq!(compression[0], 0b00001100);
        assert_eq!(compression[1], 0b00001001);
        assert_eq!(compression[2], 22 - 12);
    }

    #[test]
    fn basic_deployment_compression_test() {
        let mut initial = AccountProperties::TRIVIAL_VALUE;
        initial.balance = U256::try_from(0xFF00000000FFu64).unwrap();

        let mut bytecode = vec![1u8, 2, 3, 4, 5];
        let keccak = Keccak256::digest(&bytecode);
        let code_len = bytecode.len();

        // Add padding
        bytecode.append(&mut vec![0u8, 0u8, 0u8]);
        let blake = Blake2s256::digest(&bytecode);

        let mut r#final = AccountProperties::TRIVIAL_VALUE;
        r#final.versioning_data = VersioningData::empty_deployed();
        r#final.balance = U256::try_from(0xFF0000000000u64).unwrap();
        r#final.unpadded_code_len = code_len as u32;
        r#final.observable_bytecode_len = code_len as u32;
        r#final.bytecode_hash = blake.into();
        r#final.observable_bytecode_hash = keccak.into();

        let optimal_length =
            AccountProperties::diff_compression_length(&initial, &r#final, false).unwrap();

        let mut nop_hasher = NopHasher::new();
        let mut result_keeper = TestResultKeeper { pubdata: vec![] };
        let mut preimages_cache: BytecodeAndAccountDataPreimagesStorage<
            BaseResources<DecreasingNative>,
        > = BytecodeAndAccountDataPreimagesStorage::new_from_parts(Global);
        let mut resources: BaseResources<DecreasingNative> = BaseResources::FORMAL_INFINITE;
        preimages_cache
            .record_preimage::<false>(
                ExecutionEnvironmentType::EVM,
                &(PreimageRequest {
                    hash: r#final.bytecode_hash,
                    expected_preimage_len_in_bytes: r#final.full_bytecode_len(),
                    preimage_type: PreimageType::Bytecode,
                }),
                &mut resources,
                &[&bytecode],
            )
            .unwrap();
        let mut test_oracle = TestOracle;

        AccountProperties::diff_compression::<false, _, _>(
            &initial,
            &r#final,
            false,
            &mut nop_hasher,
            &mut result_keeper,
            &mut preimages_cache,
            &mut test_oracle,
        )
        .unwrap();
        let compression = result_keeper.pubdata;

        assert_eq!(optimal_length, compression.len() as u32);
        // full_data preimage:
        // 0b00000100 - metadata byte
        // 8 bytes versioning data
        // 1 byte nonce diff
        // 2 bytes balance diff
        // 4 bytes bytecode len
        // bytecode
        // 4 bytes observable bytecode len
        // 4 bytes artifacts len
        assert_eq!(
            compression.len() as u32,
            1 + 8 + 1 + 2 + 4 + bytecode.len() as u32 + 4 + 4
        );
        let mut expected = vec![0b00000100];
        expected.extend(r#final.versioning_data.0.to_be_bytes());
        expected.push(0b00000001); // nonce: add,initial == final == 0
        expected.push(0b00001010); // balance: sub 0xff
        expected.push(0xff); // balance: sub 0xff
        expected.extend((code_len as u32).to_be_bytes());
        expected.extend([0, 0, 0, 0]); // arifacts len
        expected.extend_from_slice(&bytecode);
        expected.extend((code_len as u32).to_be_bytes()); // observable

        assert_eq!(compression, expected);
    }
}

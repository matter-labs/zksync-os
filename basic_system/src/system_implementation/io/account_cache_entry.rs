use core::mem::MaybeUninit;
use ruint::aliases::U256;
use zk_ee::common_structs::history_map::{Appearance, CacheSnapshot};
use zk_ee::common_structs::CompressionStrategy;
use zk_ee::system::errors::InternalError;
use zk_ee::utils::Bytes32;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Default, PartialOrd, Ord, Hash)]
/// Stores multiple version information packed in u64.
/// Holds information about
/// - EE version/type (EVM, EraVM, etc.) (u8)
/// - code version (u8) - ee specific (currently both EVM and IWASM use 1)
/// - system aux bitmask (u8)
/// - EE aux bitmask (u8)
/// - deployment status (bool)
pub struct VersioningData<const DEPLOYED: u8>(u64);

impl<const DEPLOYED: u8> VersioningData<DEPLOYED> {
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

    pub const fn empty_deployed() -> Self {
        Self((DEPLOYED as u64) << 56)
    }

    pub const fn non_deployed() -> Self {
        Self(0u64)
    }

    pub const fn is_deployed(&self) -> bool {
        (self.0 >> 56) as u8 == DEPLOYED
    }

    pub fn set_as_deployed(&mut self) {
        self.0 = self.0 & 0x00ffffff_ffffffff | ((DEPLOYED as u64) << 56)
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

/// Serialization layout:
/// versioningData:               u64, LE @ [0..8] (see below)
/// nonce:                        u64, LE @ [8..16]
/// observable_bytecode_hash: Bytes32, LE @ [16..48]
/// bytecode_hash:            Bytes32, LE @ [48..80]
/// nominal_token_balance:    Bytes32, BE @ [80..112]
/// bytecode_len:                 u32, LE @ [112..116]
/// artifacts_len:                u32, LE @ [116..120]
/// observable_bytecode_len:      u32, LE @ [120..124]
///
/// VersioningData:
/// - <N>:                u8, LE @ [0..1]
/// - ee_version:         u8, LE @ [1..2]
/// - code_version:       u8, LE @ [2..3]
/// - system_aux_bitmask: u8, LE @ [3..4]
/// - ee_aux_bitmask:     u8, LE @ [4..5]
///
/// *

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct AccountProperties {
    pub versioning_data: VersioningData<DEFAULT_ADDRESS_SPECIFIC_IMMUTABLE_DATA_VERSION>,
    pub nonce: u64,
    pub observable_bytecode_hash: Bytes32,
    pub bytecode_hash: Bytes32,
    pub nominal_token_balance: U256,
    pub bytecode_len: u32,
    pub artifacts_len: u32,
    pub observable_bytecode_len: u32,
}

impl AccountProperties {
    pub const TRIVIAL_VALUE: Self = Self {
        versioning_data: VersioningData::empty_deployed(),
        nonce: 0,
        observable_bytecode_hash: Bytes32::ZERO,
        bytecode_hash: Bytes32::ZERO,
        nominal_token_balance: U256::ZERO,
        bytecode_len: 0,
        artifacts_len: 0,
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

    pub fn compute_hash(&self) -> Bytes32 {
        unsafe {
            let mut buffer = [0u32; Self::ENCODED_SIZE / core::mem::size_of::<u32>()];
            self.encode_into(buffer.as_mut_ptr().cast());
            let src = core::slice::from_raw_parts(buffer.as_ptr().cast(), Self::ENCODED_SIZE);
            use crypto::blake2s::Blake2s256;
            use crypto::MiniDigest;
            let digest = Blake2s256::digest(src);
            let mut result = Bytes32::uninit();
            let recomputed_hash = {
                result
                    .assume_init_mut()
                    .as_u8_array_mut()
                    .copy_from_slice(digest.as_slice());
                result.assume_init()
            };

            recomputed_hash
        }
    }

    pub fn encoding(&self) -> [u8; Self::ENCODED_SIZE] {
        unsafe {
            let mut buffer = [0u32; Self::ENCODED_SIZE / core::mem::size_of::<u32>()];
            self.encode_into(buffer.as_mut_ptr().cast());
            core::mem::transmute(buffer)
        }
    }

    pub fn decode(input: [u8; Self::ENCODED_SIZE]) -> Result<Self, InternalError> {
        // TODO: check invariants if needed
        // TODO: just walk over ptr

        let new = Self {
            versioning_data: VersioningData::from_u64(u64::from_le_bytes(
                <&[u8] as TryInto<[u8; 8]>>::try_into(&input[0..8]).unwrap(),
            )),
            nonce: u64::from_le_bytes(input[8..16].try_into().unwrap()),
            observable_bytecode_hash: Bytes32::from(
                <&[u8] as TryInto<[u8; 32]>>::try_into(&input[16..48]).unwrap(),
            ),
            bytecode_hash: Bytes32::from(
                <&[u8] as TryInto<[u8; 32]>>::try_into(&input[48..80]).unwrap(),
            ),
            nominal_token_balance: Bytes32::from(
                <&[u8] as TryInto<[u8; 32]>>::try_into(&input[80..112]).unwrap(),
            )
            .into_u256_be(),
            bytecode_len: u32::from_le_bytes(input[112..116].try_into().unwrap()),
            artifacts_len: u32::from_le_bytes(input[116..120].try_into().unwrap()),
            observable_bytecode_len: u32::from_le_bytes(input[120..124].try_into().unwrap()),
        };

        Ok(new)
    }

    fn encode_into(&self, mut buffer: *mut u8) {
        #[inline(always)]
        fn copy(src: &[u8], dst: &mut *mut u8) {
            unsafe {
                core::ptr::copy_nonoverlapping(src.as_ptr(), *dst, src.len());
                *dst = dst.add(src.len());
            };
        }

        copy(&self.versioning_data.into_u64().to_le_bytes(), &mut buffer);
        copy(&self.nonce.to_le_bytes(), &mut buffer);
        copy(self.observable_bytecode_hash.as_u8_ref(), &mut buffer);
        copy(self.bytecode_hash.as_u8_ref(), &mut buffer);
        copy(
            Bytes32::from_u256_be(self.nominal_token_balance).as_u8_ref(),
            &mut buffer,
        );
        copy(&self.bytecode_len.to_le_bytes(), &mut buffer);
        copy(&self.artifacts_len.to_le_bytes(), &mut buffer);
        copy(&self.observable_bytecode_len.to_le_bytes(), &mut buffer);
    }

    pub fn encode(&self, dst: &mut [MaybeUninit<u8>]) {
        self.encode_into(dst.as_mut_ptr().cast());
    }
}

type Snapshot = CacheSnapshot<AccountProperties, AccountPropertiesMetadata>;

pub(crate) fn diff(left: &Snapshot, right: &Snapshot) -> AddressDataDiff {
    fn balance_diff(left: &Snapshot, right: &Snapshot) -> Option<AddressDataDiffBalance> {
        let lv = left.value.nominal_token_balance;
        let rv = right.value.nominal_token_balance;

        let (diff, op) = match lv.cmp(&rv) {
            core::cmp::Ordering::Less => (rv - lv, CompressionStrategy::Add),
            core::cmp::Ordering::Greater => (lv - rv, CompressionStrategy::Sub),
            core::cmp::Ordering::Equal => (U256::ZERO, CompressionStrategy::Add),
        };

        if diff == U256::ZERO {
            return None;
        }

        let bytes_touched = diff.byte_len() as u8;

        Some(AddressDataDiffBalance {
            op,
            diff,
            bytes_touched,
        })
    }
    match (&left.appearance, &right.appearance) {
        (Appearance::Unset, _) => AddressDataDiff::Full,
        (_, Appearance::Updated) => {
            let balance_diff = balance_diff(left, right);
            let nonce_diff = 'arm: {
                let lv = left.value.nonce;
                let rv = right.value.nonce;

                let diff: u64 = rv - lv;

                if diff == 0 {
                    break 'arm None;
                }

                let bytes_touched = (64 - diff.leading_zeros()).next_multiple_of(8) / 8;
                let bytes_touched = bytes_touched as u8;

                Some(AddressDataDiffNonce {
                    diff,
                    bytes_touched,
                })
            };

            AddressDataDiff::Partial(AddressDataDiffPartial {
                balance: balance_diff,
                nonce: nonce_diff,
            })
        }
        // Happens due to reversible reads.
        (Appearance::Retrieved, Appearance::Retrieved) => AddressDataDiff::Empty,
        // Happens if an account with balance set, nonce and code unset is deployed
        // and deconstructed in the same tx.
        (Appearance::Retrieved, Appearance::Deconstructed) => {
            debug_assert_eq!(
                left.value.nonce, 0,
                "Deployed to an address with non-zero nonce"
            );
            debug_assert_eq!(
                left.value.bytecode_len, 0,
                "Deployed to an address with non-empty code"
            );
            let balance_diff = balance_diff(left, right);
            AddressDataDiff::Partial(AddressDataDiffPartial {
                balance: balance_diff,
                nonce: None,
            })
        }
        _ => unimplemented!(),
    }
}

pub enum AddressDataDiff {
    Full,
    Partial(AddressDataDiffPartial),
    Empty,
}

impl AddressDataDiff {
    pub(crate) fn get_encoded_size(&self) -> usize {
        match self {
            AddressDataDiff::Full => AccountProperties::ENCODED_SIZE,
            AddressDataDiff::Partial(x) => {
                match (&x.nonce, &x.balance) {
                    (None, Some(x)) => x.bytes_touched as usize + 1,
                    (Some(x), None) => x.bytes_touched as usize + 1,
                    (Some(x), Some(y)) => 2 + x.bytes_touched as usize + y.bytes_touched as usize,
                    (None, None) => 0, // Doesn't really happen.
                }
            }
            AddressDataDiff::Empty => 0,
        }
    }
}

#[allow(dead_code)]
struct AddressDataDiffBalance {
    op: CompressionStrategy,
    diff: U256,
    bytes_touched: u8,
}

#[allow(dead_code)]
struct AddressDataDiffNonce {
    diff: u64,
    bytes_touched: u8,
}

pub struct AddressDataDiffPartial {
    balance: Option<AddressDataDiffBalance>,
    nonce: Option<AddressDataDiffNonce>,
}

use crate::{
    common_traits::key_like_with_bounds::KeyLikeWithBounds, storage_types::StorageAddress,
    utils::Bytes32,
};
use ruint::aliases::B160;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct WarmStorageKey {
    pub address: B160,
    pub key: Bytes32,
}

impl PartialOrd for WarmStorageKey {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WarmStorageKey {
    /// Address limbs in order, then the key: the same order as comparing the limb array,
    /// unrolled so that no generic slice comparison runs on the map lookup path.
    #[inline(always)]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let a = self.address.as_limbs();
        let b = other.address.as_limbs();
        if a[0] != b[0] {
            return a[0].cmp(&b[0]);
        }
        if a[1] != b[1] {
            return a[1].cmp(&b[1]);
        }
        if a[2] != b[2] {
            return a[2].cmp(&b[2]);
        }
        self.key.cmp(&other.key)
    }
}

impl KeyLikeWithBounds for WarmStorageKey {
    type Subspace = B160;

    fn lower_bound(subspace: Self::Subspace) -> Self {
        Self {
            address: subspace,
            key: Bytes32::ZERO,
        }
    }

    fn upper_bound(subspace: Self::Subspace) -> Self {
        Self {
            address: subspace,
            key: Bytes32::MAX,
        }
    }
}

/// A map key assembled from an address and a slot key. Callers pass the two parts by
/// reference; the composite is only built where the map needs an owned key.
pub trait ComposedStorageKey: Ord + Clone {
    type Address: Copy;
    type Key: Copy;

    fn compose(address: &Self::Address, key: &Self::Key) -> Self;
}

impl ComposedStorageKey for WarmStorageKey {
    type Address = B160;
    type Key = Bytes32;

    #[inline(always)]
    fn compose(address: &B160, key: &Bytes32) -> Self {
        Self {
            address: *address,
            key: *key,
        }
    }
}

impl From<WarmStorageKey> for StorageAddress<crate::types_config::EthereumIOTypesConfig> {
    fn from(value: WarmStorageKey) -> Self {
        Self {
            address: value.address,
            key: value.key,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct StorageDiff {
    pub key: WarmStorageKey,
    pub previous_value: Bytes32,
}

pub fn derive_flat_storage_key(address: &B160, key: &Bytes32) -> Bytes32 {
    use crypto::blake2s::Blake2s256;
    use crypto::MiniDigest;
    let mut hasher = Blake2s256::new();
    let mut extended_address = Bytes32::ZERO;
    extended_address.as_u8_array_mut()[12..]
        .copy_from_slice(&address.to_be_bytes::<{ B160::BYTES }>());
    hasher.update(extended_address.as_u8_array_ref());
    hasher.update(key.as_u8_array_ref());
    let hash = hasher.finalize();
    Bytes32::from_array(hash)
}

pub fn derive_flat_storage_key_with_hasher(
    address: &B160,
    key: &Bytes32,
    hasher: &mut crypto::blake2s::Blake2s256,
) -> Bytes32 {
    use crypto::MiniDigest;
    hasher.update([0u8; 12]);
    hasher.update(address.to_be_bytes::<{ B160::BYTES }>());
    hasher.update(key.as_u8_array_ref());
    let hash = hasher.finalize_reset();
    Bytes32::from_array(hash)
}

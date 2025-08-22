use crate::{common_traits::key_like_with_bounds::KeyLikeWithBounds, utils::Bytes32};
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
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        match self.address.as_limbs().cmp(&other.address.as_limbs()) {
            core::cmp::Ordering::Equal => self.key.cmp(&other.key),
            a => a,
        }
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

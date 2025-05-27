use ruint::aliases::B160;
use zk_ee::{common_structs::WarmStorageKey, utils::Bytes32};

use super::KeyLikeWithBounds;

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

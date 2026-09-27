use crate::{
    common_traits::key_like_with_bounds::KeyLikeWithBounds, storage_types::StorageAddress,
    utils::Bytes32,
};
#[cfg(any(target_arch = "riscv32", test))]
use blake2s_u32::state_with_extended_control::Blake2RoundFunctionEvaluator;
use ruint::aliases::B160;
#[cfg(any(target_arch = "riscv32", test))]
use u256::byte_order::bswap32;

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

/// Derives flat storage keys, the same as `derive_flat_storage_key`. Reuse one deriver for a batch.
///
/// On riscv32 the preimage `0^12 ‖ address (big-endian) ‖ key`, exactly one 64-byte block, is
/// written as whole words into the evaluator input and hashed with one final-block round function
/// call, instead of three streaming updates and a finalization. Other targets keep the streaming
/// `Blake2s256` path (the host unit tests of this module run the riscv32 path against it).
pub struct FlatStorageKeyDeriver {
    #[cfg(any(target_arch = "riscv32", test))]
    evaluator: Blake2RoundFunctionEvaluator,
    #[cfg(not(any(target_arch = "riscv32", test)))]
    hasher: crypto::blake2s::Blake2s256,
}

impl Default for FlatStorageKeyDeriver {
    fn default() -> Self {
        Self::new()
    }
}

impl FlatStorageKeyDeriver {
    pub fn new() -> Self {
        #[cfg(any(target_arch = "riscv32", test))]
        {
            // `Blake2RoundFunctionEvaluator::new()` leaves it uninitialized on riscv32, so it is
            // zeroed instead; every derive resets the state and writes the whole input block.
            // SAFETY: every field is an integer array or a zero-size alignment marker, so all-zero
            // bytes are a valid value
            let evaluator = unsafe {
                core::mem::MaybeUninit::<Blake2RoundFunctionEvaluator>::zeroed().assume_init()
            };
            Self { evaluator }
        }

        #[cfg(not(any(target_arch = "riscv32", test)))]
        {
            use crypto::MiniDigest;
            Self {
                hasher: crypto::blake2s::Blake2s256::new(),
            }
        }
    }

    #[cfg(any(target_arch = "riscv32", test))]
    pub fn derive(&mut self, address: &B160, key: &Bytes32) -> Bytes32 {
        // Blake2s reads the block as little-endian words, so a big-endian 4-byte chunk of the
        // address is its 32-bit value byte-reversed
        let limbs = address.as_limbs();
        // SAFETY: `Bytes32` is 32 bytes of integers, so any bytes form a valid `[u32; 8]`
        let key_words =
            unsafe { core::mem::transmute::<Bytes32, [u32; 8]>(*key) }.map(u32::from_le);
        *self.evaluator.get_witness_buffer() = [
            0,
            0,
            0,
            bswap32(limbs[2] as u32),
            bswap32((limbs[1] >> 32) as u32),
            bswap32(limbs[1] as u32),
            bswap32((limbs[0] >> 32) as u32),
            bswap32(limbs[0] as u32),
            key_words[0],
            key_words[1],
            key_words[2],
            key_words[3],
            key_words[4],
            key_words[5],
            key_words[6],
            key_words[7],
        ];
        self.evaluator.reset();
        // SAFETY: the input block is fully written above
        unsafe {
            self.evaluator
                .run_round_function_with_byte_len::<false>(64, true);
        }
        let digest = self.evaluator.read_state_for_output().map(u32::to_le);
        // SAFETY: same size, and any bytes form a valid `Bytes32`
        unsafe { core::mem::transmute::<[u32; 8], Bytes32>(digest) }
    }

    #[cfg(not(any(target_arch = "riscv32", test)))]
    pub fn derive(&mut self, address: &B160, key: &Bytes32) -> Bytes32 {
        use crypto::MiniDigest;
        self.hasher.update([0u8; 12]);
        self.hasher.update(address.to_be_bytes::<{ B160::BYTES }>());
        self.hasher.update(key.as_u8_array_ref());
        let hash = self.hasher.finalize_reset();
        Bytes32::from_array(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::blake2_ext::{Blake2s256, Digest};
    use proptest::prelude::*;

    /// The preimage built byte by byte, hashed by the `blake2` crate in one call
    fn bytewise_reference(address: &B160, key: &Bytes32) -> Bytes32 {
        let mut preimage = [0u8; 64];
        preimage[12..32].copy_from_slice(&address.to_be_bytes::<{ B160::BYTES }>());
        preimage[32..].copy_from_slice(key.as_u8_array_ref());
        Bytes32::from_array(Blake2s256::digest(preimage).into())
    }

    fn check(deriver: &mut FlatStorageKeyDeriver, address: &B160, key: &Bytes32) -> Bytes32 {
        let derived = deriver.derive(address, key);
        assert_eq!(derived, derive_flat_storage_key(address, key));
        assert_eq!(derived, bytewise_reference(address, key));
        derived
    }

    const ADDRESS_MAX: B160 = B160::from_limbs([u64::MAX, u64::MAX, u32::MAX as u64]);

    /// The shape of the account-properties keys: 12 zero bytes, then an address
    const SPECIAL_KEY: Bytes32 =
        Bytes32::from_hex("000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045");

    #[test]
    fn known_answers() {
        // Python `hashlib.blake2s(bytes(12) + address + key, digest_size=32)`
        let cases = [
            (
                B160::ZERO,
                Bytes32::ZERO,
                "ae09db7cd54f42b490ef09b6bc541af688e4959bb8c53f359a6f56e38ab454a3",
            ),
            (
                B160::from_limbs([0x8003, 0, 0]),
                SPECIAL_KEY,
                "b4eb5abe8fcd2a936cd18053d1297c4d9d40f8856f656f2fd73e5dbec1d607ab",
            ),
            (
                ADDRESS_MAX,
                Bytes32::MAX,
                "74c747617242084b05f1555ac5d78b92f31fc1f180addfe7d7db9925f89e1ac0",
            ),
            (
                B160::from_be_bytes::<{ B160::BYTES }>(core::array::from_fn(|i| i as u8 + 0x01)),
                Bytes32::from_array(core::array::from_fn(|i| i as u8 + 0x21)),
                "7d29bef3ac6c4431dbc4b6d7dcd4bcca4ba383a7b3c3dc07e14c5dbcba1554fb",
            ),
        ];
        let mut deriver = FlatStorageKeyDeriver::new();
        for (address, key, expected) in cases {
            assert_eq!(
                check(&mut deriver, &address, &key),
                Bytes32::from_hex(expected)
            );
        }
    }

    #[test]
    fn edge_grid() {
        let mut addresses = vec![
            B160::ZERO,
            ADDRESS_MAX,
            B160::from_limbs([u64::MAX, 0, 0]),
            B160::from_limbs([0, 1, 0]),
            B160::from_limbs([u64::MAX, u64::MAX, 0]),
            B160::from_limbs([0, 0, 1]),
            B160::from_limbs([0x8003, 0, 0]),
        ];
        addresses.extend((0..B160::BITS).map(|bit| {
            let mut limbs = [0u64; 3];
            limbs[bit / 64] = 1 << (bit % 64);
            B160::from_limbs(limbs)
        }));
        let mut keys = vec![Bytes32::ZERO, Bytes32::MAX, SPECIAL_KEY];
        for position in 0..32 {
            for byte in [0x01, 0x80] {
                let mut key = Bytes32::ZERO;
                key.as_u8_array_mut()[position] = byte;
                keys.push(key);
            }
        }
        let mut deriver = FlatStorageKeyDeriver::new();
        for address in &addresses {
            for key in &keys {
                check(&mut deriver, address, key);
            }
        }
    }

    #[test]
    fn reused_deriver_matches_fresh_derivations() {
        // splitmix64, so the sequence is fixed without a `rand` dependency
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            seed = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = seed;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        };
        let pool: Vec<(B160, Bytes32)> = (0..256)
            .map(|_| {
                let address = B160::from_limbs([next(), next(), next() >> 32]);
                let key = Bytes32::from_array(core::array::from_fn(|_| next() as u8));
                (address, key)
            })
            .collect();
        let mut deriver = FlatStorageKeyDeriver::new();
        for i in 0..1000 {
            // a new pair, the same pair again, a fixed pair to alternate with, a random revisit
            let index = match i % 4 {
                0 | 1 => (i / 4) % pool.len(),
                2 => 0,
                _ => next() as usize % pool.len(),
            };
            let (address, key) = &pool[index];
            let derived = check(&mut deriver, address, key);
            assert_eq!(derived, FlatStorageKeyDeriver::new().derive(address, key));
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4096))]
        #[test]
        fn matches_references(address in any::<[u8; 20]>(), key in any::<[u8; 32]>()) {
            let address = B160::from_be_bytes(address);
            let key = Bytes32::from_array(key);
            let derived = FlatStorageKeyDeriver::new().derive(&address, &key);
            prop_assert_eq!(derived, derive_flat_storage_key(&address, &key));
            prop_assert_eq!(derived, bytewise_reference(&address, &key));
        }
    }
}

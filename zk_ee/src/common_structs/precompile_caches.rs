//! Caches of the results of precompiles that are pure functions of their input, for the
//! precompile calls of one `System` (one block or one simulation)

use arrayvec::ArrayVec;

/// A bounded memo of a pure function: the values of up to `CAPACITY` inputs, found by comparing
/// the whole input (`K`'s `Eq`, which must be full equality of the input; it may only choose the
/// order in which the bytes are compared). It never allocates, so inserting cannot fail;
/// when it is full, an insertion replaces the oldest entry (FIFO), so a lookup compares at most
/// `CAPACITY` inputs. Trusted mutation: the caller inserts only values it computed for that very
/// input, and only for inputs it did not find.
pub struct ExactInputCache<K, V, const CAPACITY: usize> {
    /// The entries: exactly the first `entries.len()` slots are filled (an explicit length, so
    /// zeroed memory is never taken for an entry)
    entries: ArrayVec<(K, V), CAPACITY>,
    /// Once the cache is full, the slot of the oldest entry, which the next insertion replaces
    oldest: usize,
}

impl<K: Copy + Eq, V: Copy, const CAPACITY: usize> ExactInputCache<K, V, CAPACITY> {
    pub const fn new() -> Self {
        const { assert!(CAPACITY > 0) };
        Self {
            entries: ArrayVec::new_const(),
            oldest: 0,
        }
    }

    /// The value cached for exactly `input`
    pub fn get(&self, input: &K) -> Option<&V> {
        self.entries
            .iter()
            .find(|(key, _)| key == input)
            .map(|(_, value)| value)
    }

    /// Caches `value` for `input`, which must be the value of the function for exactly this
    /// input, and `input` must not be cached already
    pub fn insert(&mut self, input: &K, value: V) {
        debug_assert!(self.get(input).is_none());
        debug_assert!(self.oldest < CAPACITY);
        if let Err(full) = self.entries.try_push((*input, value)) {
            // full: `oldest < CAPACITY == len`
            if let Some(slot) = self.entries.get_mut(self.oldest) {
                *slot = full.element();
            }
            self.oldest = if self.oldest + 1 == CAPACITY {
                0
            } else {
                self.oldest + 1
            };
        }
    }
}

impl<K: Copy + Eq, V: Copy, const CAPACITY: usize> Default for ExactInputCache<K, V, CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

/// The length of the input of the point evaluation precompile (EIP-4844)
pub const POINT_EVALUATION_INPUT_LEN: usize = 192;

/// The inputs of the point evaluation precompile that verified
pub type PointEvaluationCache = ExactInputCache<[u8; POINT_EVALUATION_INPUT_LEN], (), 1>;

/// The ecrecover precompile input after zero padding or truncation
pub const ECRECOVER_INPUT_LEN: usize = 128;

/// The normalized digest, v, r and s. Compare r and s first because signatures for the same
/// digest often share the first 64 bytes. Aligned for words: the recovery converts the
/// big-endian integers in place, by words if they are aligned (see
/// `crypto::bigint_delegation::u256::from_be_bytes`).
#[derive(Clone, Copy)]
#[repr(C, align(4))]
pub struct EcRecoverInput(pub [u8; ECRECOVER_INPUT_LEN]);

impl PartialEq for EcRecoverInput {
    fn eq(&self, other: &Self) -> bool {
        self.0[64..] == other.0[64..] && self.0[..64] == other.0[..64]
    }
}

impl Eq for EcRecoverInput {}

pub const ECRECOVER_CACHE_CAPACITY: usize = 64;
/// The recovered public-key hash, or `None` for an invalid signature
pub type EcRecoverCache =
    ExactInputCache<EcRecoverInput, Option<[u8; 32]>, ECRECOVER_CACHE_CAPACITY>;

/// Results of the precompiles that are pure functions of their input and expensive to
/// compute, for the precompile calls (from execution environments) of one `System` (one block
/// or one simulation). An entry never changes what the precompile returns or charges, only
/// whether it is recomputed. It is not rolled back with frames or transactions: a pure
/// function's value does not depend on them. The bootloader's own calls of system functions
/// (e.g. transaction signature recovery) do not use it.
///
/// A hit skips oracle queries, and the RISC-V guest replays the prover input that a native run
/// recorded, so both runs must have the same hits. The precompiles that use these caches (see
/// `SystemFunctionExt::execute_with_precompile_caches`) therefore keep these rules:
/// - whether a call hits depends only on its input and the state of the cache: never on
///   `cfg(target_arch)`, the allocator, addresses or the oracle;
/// - a hit makes no oracle queries;
/// - an input is inserted only after a complete deterministic computation for exactly that input;
///   resource and oracle errors are never cached; a computed value remains cached if
///   writing the output fails, but the output-storage error itself is not cached;
/// - a cache is never cleared, and never bypassed by a fallback.
#[derive(Default)]
pub struct PrecompileCaches {
    pub point_evaluation: PointEvaluationCache,
    pub ecrecover: EcRecoverCache,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::VecDeque;

    /// A deterministic pseudo-random sequence (xorshift64)
    struct XorShift(u64);

    impl XorShift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn key<const N: usize>(&mut self) -> [u8; N] {
            let mut key = [0u8; N];
            for byte in key.iter_mut() {
                *byte = self.next() as u8;
            }
            key
        }
    }

    fn new_cache_is_empty_for<const N: usize>() {
        let cache = ExactInputCache::<[u8; N], u32, 4>::new();
        assert_eq!(cache.get(&[0u8; N]), None);
        assert_eq!(cache.get(&[0xffu8; N]), None);
        assert_eq!(cache.get(&XorShift(N as u64 + 1).key()), None);
        let cache = ExactInputCache::<[u8; N], (), 1>::default();
        assert_eq!(cache.get(&[0u8; N]), None);
    }

    #[test]
    fn new_cache_is_empty() {
        new_cache_is_empty_for::<1>();
        new_cache_is_empty_for::<32>();
        new_cache_is_empty_for::<128>();
        new_cache_is_empty_for::<POINT_EVALUATION_INPUT_LEN>();
    }

    fn insert_then_get_for<const N: usize>() {
        let mut rng = XorShift(0x1234 + N as u64);
        let key = rng.key::<N>();
        let mut other = key;
        other[N - 1] ^= 1;
        let mut cache = ExactInputCache::<[u8; N], u32, 1>::new();
        cache.insert(&key, 7);
        assert_eq!(cache.get(&key), Some(&7));
        assert_eq!(cache.get(&other), None);
        assert_eq!(cache.get(&[0u8; N]), None);
        let mut cache = PointEvaluationCache::new();
        let key = rng.key::<POINT_EVALUATION_INPUT_LEN>();
        cache.insert(&key, ());
        assert_eq!(cache.get(&key), Some(&()));
        assert_eq!(cache.get(&[0u8; POINT_EVALUATION_INPUT_LEN]), None);
    }

    #[test]
    fn insert_then_get() {
        insert_then_get_for::<1>();
        insert_then_get_for::<32>();
        insert_then_get_for::<POINT_EVALUATION_INPUT_LEN>();
    }

    /// Needs `N >= 3`, for three distinct bytes to vary
    fn full_key_compare_for<const N: usize>() {
        let base = XorShift(0x5678 + N as u64).key::<N>();
        let variants = [0, N / 2, N - 1].map(|index| {
            let mut key = base;
            key[index] ^= 0x80;
            key
        });
        let mut cache = ExactInputCache::<[u8; N], usize, 4>::new();
        cache.insert(&base, 100);
        for (value, key) in variants.iter().enumerate() {
            assert_eq!(cache.get(key), None, "byte {value} of {N}");
            cache.insert(key, value);
        }
        assert_eq!(cache.get(&base), Some(&100));
        for (value, key) in variants.iter().enumerate() {
            assert_eq!(cache.get(key), Some(&value));
        }
        // a single-entry cache tells a key from each of its one-byte variants too
        let mut cache = ExactInputCache::<[u8; N], (), 1>::new();
        cache.insert(&base, ());
        for key in variants.iter() {
            assert_eq!(cache.get(key), None);
        }
        assert_eq!(cache.get(&base), Some(&()));
    }

    #[test]
    fn full_key_compare() {
        full_key_compare_for::<3>();
        full_key_compare_for::<33>();
        full_key_compare_for::<POINT_EVALUATION_INPUT_LEN>();
    }

    fn fifo_eviction_for<const CAPACITY: usize>() {
        let keys: alloc::vec::Vec<[u8; 16]> = (0..2 * CAPACITY + 1)
            .map(|index| {
                let mut key = [0u8; 16];
                key[15] = index as u8;
                key
            })
            .collect();
        let mut cache = ExactInputCache::<[u8; 16], usize, CAPACITY>::new();
        for (index, key) in keys.iter().enumerate() {
            cache.insert(key, index);
            // the last `CAPACITY` insertions are present, the older ones are not
            for (other_index, other) in keys[..=index].iter().enumerate() {
                let expected = (other_index + CAPACITY > index).then_some(&other_index);
                assert_eq!(cache.get(other), expected, "{other_index} after {index}");
            }
            assert!(cache.entries.len() <= CAPACITY);
        }
    }

    #[test]
    fn fifo_eviction() {
        fifo_eviction_for::<1>();
        fifo_eviction_for::<2>();
        fifo_eviction_for::<4>();
    }

    fn model_equivalence_for<const CAPACITY: usize>() {
        let mut rng = XorShift(0x9e37_79b9_7f4a_7c15 ^ CAPACITY as u64);
        let key_space: alloc::vec::Vec<[u8; 8]> =
            (0..3 * CAPACITY + 1).map(|_| rng.key()).collect();
        let mut cache = ExactInputCache::<[u8; 8], u64, CAPACITY>::new();
        let mut model = VecDeque::<([u8; 8], u64)>::new();
        for step in 0..10_000u64 {
            let key = &key_space[rng.next() as usize % key_space.len()];
            let expected = model
                .iter()
                .find(|(model_key, _)| model_key == key)
                .map(|(_, value)| value);
            let found = cache.get(key);
            assert_eq!(found, expected, "step {step}");
            // the contract: only inputs that were not found are inserted
            if found.is_none() && rng.next() & 1 == 0 {
                cache.insert(key, step);
                if model.len() == CAPACITY {
                    model.pop_front();
                }
                model.push_back((*key, step));
            }
            assert!(cache.entries.len() <= CAPACITY);
            assert_eq!(cache.entries.len(), model.len());
        }
    }

    #[test]
    fn model_equivalence() {
        model_equivalence_for::<1>();
        model_equivalence_for::<3>();
        model_equivalence_for::<64>();
    }

    #[test]
    fn ecrecover_key_compares_every_byte() {
        let base = EcRecoverInput(XorShift(29).key());
        assert!(base == base);
        for index in 0..ECRECOVER_INPUT_LEN {
            let mut other = base;
            other.0[index] ^= 1;
            assert!(base != other, "byte {index}");
        }
        let mut rng = XorShift(0x29_29_29);
        for _ in 0..4096 {
            let a = EcRecoverInput(rng.key());
            let mut b = a;
            let index = rng.next() as usize % ECRECOVER_INPUT_LEN;
            b.0[index] ^= 1;
            assert_eq!(a == b, a.0 == b.0);
        }
    }

    #[test]
    fn ecrecover_cache_negative_result_and_fifo() {
        let mut cache = EcRecoverCache::new();
        let key = |index: u8| {
            let mut bytes = [0; ECRECOVER_INPUT_LEN];
            bytes[127] = index;
            EcRecoverInput(bytes)
        };
        assert_eq!(cache.get(&key(0)), None);
        cache.insert(&key(0), None);
        assert_eq!(cache.get(&key(0)), Some(&None));
        for index in 1..=128 {
            cache.insert(&key(index), Some([index; 32]));
            for previous in 0..=index {
                let present = usize::from(previous) + ECRECOVER_CACHE_CAPACITY > usize::from(index);
                assert_eq!(cache.get(&key(previous)).is_some(), present);
            }
        }
        for index in 65..=128 {
            assert_eq!(cache.get(&key(index)), Some(&Some([index; 32])));
        }
    }
}

//! A cheap hasher for keys that are a few machine words (addresses, slot
//! keys, small integers): every word is folded into a 32-bit state with one
//! multiply, which is what the proving target does well. Not a defense against
//! adversarial collisions: the keys it is meant for are dense indices or
//! already hash-derived values, and a hash map degrades, it does not break.

use core::hash::{BuildHasher, Hasher};

/// Odd multiplier of the multiplicative (Fibonacci) hash
const MULTIPLIER: u32 = 0x9E37_79B1;

#[derive(Clone, Copy, Default)]
pub struct WordHasher(u32);

impl WordHasher {
    #[inline(always)]
    fn mix(&mut self, word: u32) {
        self.0 = (self.0 ^ word).wrapping_mul(MULTIPLIER);
    }
}

impl Hasher for WordHasher {
    /// The avalanched 32-bit state in both halves: a table that takes its
    /// bucket from the low bits and its tag from the top bits gets independent,
    /// well-mixed bits for each. The fold alone leaves the low bits of the
    /// state a function of the low bits of the words only, which for
    /// structured keys (dense indices packed in halves, small integers) puts
    /// hundreds of keys in one bucket; the final mix (murmur3's) spreads them.
    #[inline(always)]
    fn finish(&self) -> u64 {
        let mut x = self.0;
        x ^= x >> 16;
        x = x.wrapping_mul(0x85EB_CA6B);
        x ^= x >> 13;
        x = x.wrapping_mul(0xC2B2_AE35);
        x ^= x >> 16;
        ((x as u64) << 32) | (x as u64)
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<4>();
        for chunk in chunks {
            self.mix(u32::from_le_bytes(*chunk));
        }
        if !rest.is_empty() {
            let mut tail = [0u8; 4];
            tail[..rest.len()].copy_from_slice(rest);
            self.mix(u32::from_le_bytes(tail));
        }
    }

    #[inline(always)]
    fn write_u8(&mut self, i: u8) {
        self.mix(i as u32);
    }

    #[inline(always)]
    fn write_u16(&mut self, i: u16) {
        self.mix(i as u32);
    }

    #[inline(always)]
    fn write_u32(&mut self, i: u32) {
        self.mix(i);
    }

    #[inline(always)]
    fn write_u64(&mut self, i: u64) {
        self.mix(i as u32);
        self.mix((i >> 32) as u32);
    }

    #[inline(always)]
    fn write_usize(&mut self, i: usize) {
        if core::mem::size_of::<usize>() == 8 {
            self.write_u64(i as u64);
        } else {
            self.mix(i as u32);
        }
    }
}

/// Builds [`WordHasher`]s: stateless, so every map hashes the same way
#[derive(Clone, Copy, Default, Debug)]
pub struct BuildWordHasher;

impl BuildHasher for BuildWordHasher {
    type Hasher = WordHasher;

    #[inline(always)]
    fn build_hasher(&self) -> WordHasher {
        WordHasher(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::hash::Hash;

    fn hash_of<T: Hash>(value: &T) -> u64 {
        BuildWordHasher.hash_one(value)
    }

    #[test]
    fn distinct_words_hash_apart_and_the_tag_bits_vary() {
        let hashes: std::vec::Vec<u64> = (0u32..1000).map(|i| hash_of(&i)).collect();
        let distinct: std::collections::BTreeSet<u64> = hashes.iter().copied().collect();
        assert_eq!(distinct.len(), hashes.len());
        let tags: std::collections::BTreeSet<u64> = hashes.iter().map(|h| h >> 57).collect();
        assert!(tags.len() > 64, "the top bits are not stuck");
        assert_ne!(hash_of(&[1u64, 0]), hash_of(&[0u64, 1]), "position matters");
    }

    #[test]
    fn byte_slices_hash_like_their_words() {
        let mut a = WordHasher::default();
        a.write(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let mut b = WordHasher::default();
        b.write_u32(u32::from_le_bytes([1, 2, 3, 4]));
        b.write_u32(u32::from_le_bytes([5, 6, 7, 8]));
        assert_eq!(a.finish(), b.finish());
        let mut c = WordHasher::default();
        c.write(&[1, 2, 3]);
        let mut d = WordHasher::default();
        d.write_u32(u32::from_le_bytes([1, 2, 3, 0]));
        assert_eq!(c.finish(), d.finish());
    }
}

/// Collision study on realistic key families, with hashbrown itself: the
/// number of key comparisons per successful lookup (every tag match in the
/// probe sequence compares keys) and the share of keys whose home bucket is
/// shared. Prints a table; run with `--ignored --nocapture`.
#[cfg(test)]
mod collision_study {
    use super::*;
    use core::cell::Cell;
    use core::hash::{Hash, Hasher};
    use hashbrown::HashMap;

    thread_local! { static EQ_CALLS: Cell<u64> = const { Cell::new(0) }; }

    #[derive(Clone, Copy)]
    struct Counted<K>(K);
    impl<K: PartialEq> PartialEq for Counted<K> {
        fn eq(&self, other: &Self) -> bool {
            EQ_CALLS.with(|c| c.set(c.get() + 1));
            self.0 == other.0
        }
    }
    impl<K: PartialEq> Eq for Counted<K> {}
    impl<K: Hash> Hash for Counted<K> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.0.hash(state)
        }
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    /// Words hashed one by one, like `AddressKey`/`SlotKey`
    #[derive(Clone, Copy, PartialEq, Eq)]
    struct Words<const N: usize>([u64; N]);
    impl<const N: usize> Hash for Words<N> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            for w in &self.0 {
                state.write_u64(*w);
            }
        }
    }

    /// Returns (key compares per lookup, largest home bucket load)
    fn study<K: Copy + Hash + Eq, S: BuildHasher + Clone>(
        name: &str,
        keys: &[K],
        hasher: S,
    ) -> (f64, u32) {
        let mut map: HashMap<Counted<K>, u32, S> =
            HashMap::with_capacity_and_hasher(keys.len(), hasher.clone());
        for (i, k) in keys.iter().enumerate() {
            map.insert(Counted(*k), i as u32);
        }
        EQ_CALLS.with(|c| c.set(0));
        for k in keys {
            assert!(map.get(&Counted(*k)).is_some());
        }
        let compares = EQ_CALLS.with(|c| c.get()) as f64 / keys.len() as f64;
        // home bucket sharing, with hashbrown's bucket count and low-bits bucket choice
        let buckets = (keys.len() * 8 / 7).next_power_of_two();
        let mut load = std::vec![0u32; buckets];
        for k in keys {
            load[(hasher.hash_one(k) as usize) & (buckets - 1)] += 1;
        }
        let shared: u32 = load.iter().filter(|&&n| n > 1).sum();
        let worst = *load.iter().max().unwrap();
        std::println!(
            "{name:34} compares/lookup {compares:5.3}  keys in shared home bucket {:5.1}%  max home load {worst}",
            100.0 * shared as f64 / keys.len() as f64
        );
        (compares, worst)
    }

    fn families() -> std::vec::Vec<(
        &'static str,
        std::vec::Vec<u32>,
        std::vec::Vec<Words<3>>,
        std::vec::Vec<Words<4>>,
    )> {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        // packed slot cache keys: 300 accounts x 100 slot indices, then 30k dense indices
        let packed: std::vec::Vec<u32> = (0..300u32)
            .flat_map(|a| (0..100u32).map(move |s| (a << 16) | s))
            .collect();
        // addresses: keccak-like, plus 5% vanity (leading zero bytes) and the precompiles
        let mut addresses: std::vec::Vec<Words<3>> = (0..28_500)
            .map(|_| Words([rng.next(), rng.next(), rng.next() & 0xffff_ffff]))
            .collect();
        for i in 0..1_500u64 {
            addresses.push(Words([rng.next() & 0xffff_ffff_ffff, i, 0])); // 0x0000...-prefixed
        }
        for i in 1..=10u64 {
            addresses.push(Words([i, 0, 0]));
        }
        // slot keys: half keccak-derived, half small integers (as the big-endian bytes of a U256:
        // the value sits in the last word, and as little-endian: in the first)
        let mut slots: std::vec::Vec<Words<4>> = (0..15_000)
            .map(|_| Words([rng.next(), rng.next(), rng.next(), rng.next()]))
            .collect();
        for i in 0..7_500u64 {
            slots.push(Words([0, 0, 0, (i + 1).swap_bytes()]));
            slots.push(Words([i + 1, 0, 0, 0]));
        }
        std::vec![("packed u32 (300x100)", packed, addresses, slots)]
    }

    /// The hasher must not do worse than a strong hash on the key families the
    /// storage model uses: at most one and a bit key compares per lookup, and no
    /// bucket that gathers more than a handful of keys.
    #[test]
    fn structured_keys_spread_over_the_buckets() {
        let (_, packed, addresses, slots) = families().remove(0);
        let dense: std::vec::Vec<u32> = (0..30_000).collect();
        assert!(study("packed keys", &packed, BuildWordHasher) <= (1.05, 8));
        assert!(study("dense u32", &dense, BuildWordHasher) <= (1.05, 8));
        assert!(study("addresses", &addresses, BuildWordHasher) <= (1.05, 8));
        assert!(study("slot keys", &slots, BuildWordHasher) <= (1.05, 8));
    }

    #[test]
    #[ignore]
    fn print_collision_study() {
        for (name, packed, addresses, slots) in families() {
            std::println!("--- {name}");
            study("packed keys / WordHasher", &packed, BuildWordHasher);
            study(
                "packed keys / SipHash",
                &packed,
                std::collections::hash_map::RandomState::new(),
            );
            let dense: std::vec::Vec<u32> = (0..30_000).collect();
            study("dense u32 / WordHasher", &dense, BuildWordHasher);
            study(
                "dense u32 / SipHash",
                &dense,
                std::collections::hash_map::RandomState::new(),
            );
            study("addresses / WordHasher", &addresses, BuildWordHasher);
            study(
                "addresses / SipHash",
                &addresses,
                std::collections::hash_map::RandomState::new(),
            );
            study("slot keys / WordHasher", &slots, BuildWordHasher);
            study(
                "slot keys / SipHash",
                &slots,
                std::collections::hash_map::RandomState::new(),
            );
        }
    }
}

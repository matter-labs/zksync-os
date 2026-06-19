//! Ethereum logs bloom filter (the 2048-bit `logsBloom` field).
//!
//! Shared between the Ethereum block flow (per-tx and block-level blooms) and
//! the ZK receipt encoding. The bit-setting algorithm matches the Ethereum
//! Yellow Paper / go-ethereum `bloom9`: for the keccak256 of each item (the log
//! address and every topic), take the low 11 bits of the first three big-endian
//! 2-byte words and set those bits in the 256-byte filter.

use crypto::MiniDigest;
use zk_ee::common_structs::GenericEventContentRef;
use zk_ee::system::MAX_EVENT_TOPICS;
use zk_ee::types_config::EthereumIOTypesConfig;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogsBloom {
    inner: [u8; 256], // the 2048-bit filter, in Ethereum byte order
}

// `[u8; 256]` does not derive `Default` (the std impl only covers arrays up to
// length 32), so spell out the all-zero filter by hand.
impl Default for LogsBloom {
    fn default() -> Self {
        Self { inner: [0u8; 256] }
    }
}

impl LogsBloom {
    pub fn from_bytes(input: &[u8; 256]) -> Self {
        Self { inner: *input }
    }

    pub fn as_bytes(&self) -> &[u8; 256] {
        &self.inner
    }
    fn as_bytes_mut(&mut self) -> &mut [u8; 256] {
        &mut self.inner
    }
    pub fn mark_events<'a>(
        &mut self,
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
        events: impl Iterator<
            Item = GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
        >,
    ) {
        for event in events {
            self.mark_event(hasher, event);
        }
    }

    pub fn mark_event<'a>(
        &mut self,
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
        event: GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
    ) {
        hasher.update(&event.address.to_be_bytes::<20>());
        let address_hash = hasher.finalize_reset();
        self.mark(&address_hash);
        for topic in event.topics.iter() {
            hasher.update(topic.as_u8_ref());
            let topic_hash = hasher.finalize_reset();
            self.mark(&topic_hash);
        }
    }

    fn mark(&mut self, hash: &[u8; 32]) {
        // take lowest 11 bits integer of each of 2-byte words BE words
        for i in [0, 2, 4] {
            let word = [hash[i], hash[i + 1]];
            let word = (u16::from_be_bytes(word) & 0x7ff) as usize; // equal to mod 2048
            let byte_idx = word / 8;
            let bit_idx = word % 8;
            self.as_bytes_mut()[255 - byte_idx] |= 1 << bit_idx;
        }
    }

    pub fn merge(&mut self, other: &Self) {
        for (dst, src) in self.inner.iter_mut().zip(other.inner.iter()) {
            *dst |= *src;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, Bloom, Bytes, Log, B256};
    use arrayvec::ArrayVec;
    use crypto::sha3::Keccak256;
    use ruint::aliases::B160;
    use zk_ee::utils::Bytes32;

    /// Known-vector check: `mark_event` must set exactly the bits alloy's
    /// canonical bloom does (`Bloom::accrue_log`, i.e. the Yellow Paper M3:2048
    /// over the keccak256 of the log address and of every topic).
    #[test]
    fn mark_event_matches_alloy_bloom() {
        let addr_bytes = [0xaau8; 20];
        let topic_bytes = [[0x11u8; 32], [0x22u8; 32], [0x33u8; 32], [0x44u8; 32]];

        // Exercise 0..=MAX topics so every topic contributes to the filter.
        for n_topics in 0..=topic_bytes.len() {
            let addr = B160::from_be_bytes::<20>(addr_bytes);
            let mut topics: ArrayVec<Bytes32, MAX_EVENT_TOPICS> = ArrayVec::new();
            for t in &topic_bytes[..n_topics] {
                topics.push(Bytes32::from_array(*t));
            }
            let data: [u8; 0] = [];
            let event: GenericEventContentRef<'_, MAX_EVENT_TOPICS, EthereumIOTypesConfig> =
                GenericEventContentRef {
                    address: &addr,
                    topics: &topics,
                    data: &data,
                };

            let mut ours = LogsBloom::default();
            let mut hasher = Keccak256::new();
            ours.mark_event(&mut hasher, event);

            let alloy_log = Log::new_unchecked(
                Address::from(addr_bytes),
                topic_bytes[..n_topics]
                    .iter()
                    .map(|t| B256::from(*t))
                    .collect(),
                Bytes::new(),
            );
            let mut reference = Bloom::default();
            reference.accrue_log(&alloy_log);

            assert_eq!(
                ours.as_bytes().as_slice(),
                reference.as_slice(),
                "bloom mismatch for {n_topics} topics"
            );
        }
    }

    /// `merge` is the bitwise OR of the two filters, and round-trips bytes.
    #[test]
    fn merge_is_bitwise_or() {
        let mut a = LogsBloom::from_bytes(&[0b1010_1010u8; 256]);
        let b = LogsBloom::from_bytes(&[0b0101_0101u8; 256]);
        a.merge(&b);
        assert_eq!(a.as_bytes(), &[0xffu8; 256]);
    }
}

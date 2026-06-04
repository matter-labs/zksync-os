//! Ethereum logs bloom filter (the 2048-bit `logsBloom` field).
//!
//! Shared between the Ethereum block flow (per-tx and block-level blooms) and
//! the ZK receipt encoding. The bit-setting algorithm matches the Ethereum
//! Yellow Paper / go-ethereum `bloom9`: for the keccak256 of each item (the log
//! address and every topic), take the low 11 bits of the first three big-endian
//! 2-byte words and set those bits in the 256-byte filter.

use core::ptr::addr_of_mut;
use crypto::MiniDigest;
use zk_ee::common_structs::GenericEventContentRef;
use zk_ee::system::MAX_EVENT_TOPICS;
use zk_ee::types_config::EthereumIOTypesConfig;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogsBloom {
    inner: [u64; 32], // blindly the capacity for 2048 bits, treated as BE integer all together
}

impl LogsBloom {
    pub fn from_bytes(input: &[u8; 256]) -> Self {
        unsafe {
            let mut result = core::mem::MaybeUninit::<Self>::uninit();
            core::ptr::write(
                addr_of_mut!((*result.as_mut_ptr()).inner).cast::<[u8; 256]>(),
                *input,
            );

            result.assume_init()
        }
    }

    pub fn as_bytes(&self) -> &[u8; 256] {
        // We are overaligned and continuous
        unsafe { core::mem::transmute(self) }
    }
    fn as_bytes_mut(&mut self) -> &mut [u8; 256] {
        // We are overaligned and continuous
        unsafe { core::mem::transmute(self) }
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

            // let u64_idx = 31 - word / 64; // BE
            // let bit_idx = 63 - word % 64; // BE
            // self.inner[u64_idx] |= 1 << bit_idx;
        }
    }

    pub fn merge(&mut self, other: &Self) {
        for (dst, src) in self.inner.iter_mut().zip(other.inner.iter()) {
            *dst |= *src;
        }
    }
}

#![cfg_attr(all(not(feature = "evaluate"), not(test)), no_std)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![allow(clippy::new_without_default)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::double_must_use)]
#![allow(clippy::explicit_auto_deref)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::borrow_deref_ref)]
#![allow(clippy::op_ref)]
#![allow(clippy::precedence)]

pub mod arithmetic;
pub mod blob_kzg_commitment;
pub mod field_hints;

/// Shared test utilities for callable oracle unit tests.
#[cfg(any(test, feature = "testing"))]
pub mod test_utils {
    use oracle_provider::RamPeek;
    use std::collections::BTreeMap;

    /// BTreeMap-backed memory source for testing oracle query processors.
    #[derive(Default)]
    pub struct TestMemorySource {
        words: BTreeMap<u32, u32>,
    }

    impl TestMemorySource {
        pub fn insert_u32(&mut self, address: u32, value: u32) {
            assert!(address.is_multiple_of(4));
            self.words.insert(address, value);
        }

        /// Write a byte slice into memory starting at the given word-aligned offset.
        /// Data is written in little-endian order, 4 bytes per word.
        /// Partial final chunks are zero-padded.
        pub fn write_bytes(&mut self, offset: u32, data: &[u8]) {
            for (i, chunk) in data.chunks(4).enumerate() {
                let mut word = [0u8; 4];
                word[..chunk.len()].copy_from_slice(chunk);
                let val = u32::from_le_bytes(word);
                let addr = offset + (i as u32) * 4;
                self.insert_u32(addr, val);
            }
        }
    }

    impl RamPeek for TestMemorySource {
        fn peek_word(&self, address: u32) -> u32 {
            self.words.get(&address).copied().unwrap_or(0)
        }
    }
}

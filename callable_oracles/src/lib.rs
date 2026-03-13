#![cfg_attr(all(not(feature = "evaluate"), not(test)), no_std)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(iter_array_chunks)]
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

#[cfg(all(
    not(target_arch = "riscv32"),
    not(all(target_pointer_width = "64", target_endian = "little"))
))]
compile_error!("native callable oracle host helpers require a 64-bit little-endian host target");

pub mod arithmetic;
pub mod blob_kzg_commitment;
pub mod field_hints;
pub mod utils;

use zk_ee::{
    oracle::usize_serialization::{WordDeserializable, WordSerializable, WordSink},
    system::errors::internal::InternalError,
};

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

#[derive(Clone, Copy, Debug)]
pub struct MemoryRegionDescriptionParams {
    pub offset: u32,
    pub len: u32,
}

impl WordSerializable for MemoryRegionDescriptionParams {
    fn word_len(&self) -> usize {
        <u32 as WordSerializable>::word_len(&self.offset)
            + <u32 as WordSerializable>::word_len(&self.len)
    }

    fn write_words(&self, out: &mut impl WordSink) {
        <u32 as WordSerializable>::write_words(&self.offset, out);
        <u32 as WordSerializable>::write_words(&self.len, out);
    }
}

impl WordDeserializable for MemoryRegionDescriptionParams {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let offset = <u32 as WordDeserializable>::read_words(src)?;
        let len = <u32 as WordDeserializable>::read_words(src)?;

        let new = Self { offset, len };

        Ok(new)
    }
}

/// Convert a host-supplied integer address into a raw pointer after the
/// structural checks we can do locally.
///
/// This validates that the address is non-null, fits into the current
/// process's `usize`, and satisfies the requested alignment.
fn validate_host_pointer(ptr_u64: u64, alignment: usize) -> *const u8 {
    assert!(ptr_u64 != 0);
    let addr = usize::try_from(ptr_u64).unwrap();
    assert!(addr.is_multiple_of(alignment));

    addr as *const u8
}

fn checked_byte_len(len_units: u64, bytes_per_unit: usize) -> usize {
    let len_units = usize::try_from(len_units).unwrap();
    len_units.checked_mul(bytes_per_unit).unwrap()
}

#[inline(always)]
pub(crate) fn read_u64_words(ptr_u64: u64, len_words_u64: u64) -> Vec<u64> {
    if len_words_u64 == 0 {
        return vec![];
    }
    let ptr = validate_host_pointer(ptr_u64, core::mem::align_of::<u64>());
    let len_bytes = checked_byte_len(len_words_u64, core::mem::size_of::<u64>());
    let len_words = len_bytes / core::mem::size_of::<u64>();

    // Safety: `ptr` was validated to be non-null and aligned for `u64`, and
    // `len_words` was derived from a checked byte-length computation. The
    // caller guarantees that the pointed-to region is fully initialized,
    // readable for `len_words` elements, remains live for the duration of
    // this read, and is not concurrently mutated while the slice exists.
    let words = unsafe { core::slice::from_raw_parts(ptr.cast::<u64>(), len_words) };
    words.to_vec()
}

#[inline(always)]
pub(crate) fn read_u8_words(ptr_u64: u64, len_words_u8: u64) -> Vec<u8> {
    if len_words_u8 == 0 {
        return vec![];
    }
    let ptr = validate_host_pointer(ptr_u64, core::mem::align_of::<u8>());
    let len_bytes = checked_byte_len(len_words_u8, core::mem::size_of::<u8>());

    // Safety: `ptr` was validated to be non-null, and `len_bytes` was
    // derived from a checked byte-length computation. The caller guarantees
    // that the pointed-to region is fully initialized, readable for
    // `len_bytes` bytes, remains live for the duration of this read, and is
    // not concurrently mutated while the slice exists.
    let bytes = unsafe { core::slice::from_raw_parts(ptr, len_bytes) };
    bytes.to_vec()
}

#[inline(always)]
pub(crate) fn read_host_struct<T: Copy>(ptr_u64: u64) -> T {
    let ptr = validate_host_pointer(ptr_u64, core::mem::align_of::<T>());

    // Safety: `ptr` was validated to be non-null and aligned for `T`. The
    // caller guarantees that it points to a fully initialized `T` in the
    // current process address space, that the value remains live for the
    // duration of this read, and that the memory is not concurrently
    // mutated while it is being read.
    unsafe { ptr.cast::<T>().read() }
}

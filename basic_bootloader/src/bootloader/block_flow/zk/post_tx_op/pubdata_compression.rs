//! In-STF deflate compression for the v3 pubdata body.
//!
//! The v2 layout is `[VERSION:1][BLOCK_HASH:32][TIMESTAMP:8][BODY...]`. v3
//! keeps the fixed-size header uncompressed (so consumers can read block_hash
//! and timestamp without inflating) and replaces `BODY` with its DEFLATE
//! encoding. The deflate stream is self-terminating; no length prefix is
//! emitted.
//!
//! Both forward and proving execution paths run this code, and miniz's
//! deflate is fully deterministic for fixed flags, so the two paths emit
//! identical bytes for identical inputs.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::alloc::Allocator;

use miniz_nostd_compression::deflate::core::CompressorOxideInner;
use miniz_nostd_compression::deflate::{
    compress_flags_default, compress_to_buffer, HashBuffers, HuffmanOxide, LocalBuf,
};

/// miniz compression level used in-circuit.
///
/// 1 (= fastest) skips lazy matching and full hash-chain probing; on the
/// measurement workloads it lost only ~3 percentage points of ratio vs
/// level 9 (e.g. 48.7% vs 45.0% for a deploy-heavy 3.7 KB block). That trade
/// is the right call in-circuit: every additional probe costs RISC-V cycles
/// and the cycle delta dominates the byte delta.
pub const COMPRESSION_LEVEL: u8 = 1;

/// Tight upper bound on deflate output: zlib's `deflateBound` is
/// `n + (n>>12) + (n>>14) + (n>>25) + 13`. We use `n + n/8 + 64` which is
/// strictly larger for all n and keeps the math cheap.
fn deflate_output_cap(input_len: usize) -> usize {
    input_len + input_len / 8 + 64
}

/// Allocate a zero-initialized `T` directly on the heap via `allocator`.
/// Avoids a giant stack copy that `Box::new_in(T::default(), allocator)`
/// would do for the 160 KB+ deflate scratch buffers.
///
/// # Safety
/// Caller must guarantee that an all-zero bit pattern is a valid value of T.
/// All three deflate scratch types (`HuffmanOxide`, `LocalBuf`, `HashBuffers`)
/// satisfy this: each is a record of fixed-size arrays of integer types whose
/// `Default::default()` is the all-zero pattern.
unsafe fn boxed_zeroed_in<T, A: Allocator>(allocator: A) -> Box<T, A> {
    Box::<T, A>::new_zeroed_in(allocator).assume_init()
}

/// Deflate `input` into a freshly-allocated buffer on the given allocator.
///
/// The returned `Vec` holds raw deflate bytes (no zlib wrapper). Panics if
/// the compressor reports failure — under the calling convention here the
/// output buffer is sized to `deflate_output_cap(input.len())` which is a
/// proven upper bound on deflate output, so the failure path is unreachable
/// in practice. Treating it as an internal invariant violation rather than
/// a recoverable error keeps `write_pubdata`'s `()` return type intact.
pub fn deflate_pubdata_body<A: Allocator + Clone>(input: &[u8], allocator: A) -> Vec<u8, A> {
    // SAFETY: each of these three types has an all-zero bit pattern as its
    // `Default`; see `boxed_zeroed_in` doc.
    let mut huff: Box<HuffmanOxide, A> = unsafe { boxed_zeroed_in(allocator.clone()) };
    let mut local_buf: Box<LocalBuf, A> = unsafe { boxed_zeroed_in(allocator.clone()) };
    let mut hb: Box<HashBuffers, A> = unsafe { boxed_zeroed_in(allocator.clone()) };

    let flags = compress_flags_default(COMPRESSION_LEVEL);
    let mut compressor = CompressorOxideInner::new(flags, &mut huff, &mut local_buf, &mut hb);

    let mut output = Vec::with_capacity_in(deflate_output_cap(input.len()), allocator);
    // `compress_to_buffer` writes into `&mut [u8]`, so we need the spare
    // capacity to be addressable as a slice. Resize-to-cap with 0 bytes (no
    // memcpy of pre-existing data; this is the first fill of the buffer).
    output.resize(output.capacity(), 0u8);

    let used = compress_to_buffer(&mut compressor, input, &mut output)
        .expect("deflate output buffer underestimated — deflate_output_cap is wrong");
    output.truncate(used);
    output
}

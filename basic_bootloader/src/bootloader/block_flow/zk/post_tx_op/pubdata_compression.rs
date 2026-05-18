//! In-STF streaming deflate compression for the v2 pubdata body.
//!
//! Layout: `[VERSION:1][BLOCK_HASH:32][TIMESTAMP:8][DEFLATE(BODY)]`. The
//! 41-byte fixed header stays uncompressed (so consumers can read
//! block_hash and timestamp without inflating); `BODY` (storage diffs +
//! logs + messages) is DEFLATE-encoded. The deflate stream is
//! self-terminating; no length prefix is emitted.
//!
//! Compression runs *streaming*: the body emitters call
//! `DeflateSink::write(buf)` directly, miniz consumes the bytes and emits
//! compressed output into a small fixed-size buffer, which is drained to
//! both `pubdata_dst` (DA commitment) and `result_keeper` on the fly.
//!
//! This avoids ever materialising the full body or the full compressed
//! output as a contiguous buffer — peak memory is `scratch (~254 KB) +
//! out_buf (16 KB)` instead of `scratch + body (1.1 MB) + output_cap
//! (~1.2 MB)`. The proving allocator (`proof_running_system::ProxyAllocator`)
//! panics on `grow`, so eliminating the resizable Vecs also removes a class
//! of bugs.
//!
//! Both forward and proving execution paths run this code, and miniz's
//! deflate is fully deterministic for fixed flags, so the two paths emit
//! identical bytes for identical inputs.

use alloc::boxed::Box;
use core::alloc::Allocator;

use miniz_nostd_compression::deflate::compress_flags_default;
use miniz_nostd_compression::deflate::core::{
    compress, CompressorOxideInner, TDEFLFlush, TDEFLStatus,
};
use zk_ee::system::IOResultKeeper;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::write_bytes::WriteBytes;

// Re-export the scratch types so `write_pubdata` can allocate them on the
// system heap without depending on `miniz_nostd_compression` directly.
pub use miniz_nostd_compression::deflate::{HashBuffers, HuffmanOxide, LocalBuf};

/// miniz compression level used in-circuit.
///
/// 1 (= fastest) skips lazy matching and full hash-chain probing; on the
/// measurement workloads it lost only ~3 percentage points of ratio vs
/// level 9 (e.g. 48.7% vs 45.0% for a deploy-heavy 3.7 KB block). That trade
/// is the right call in-circuit: every additional probe costs RISC-V cycles
/// and the cycle delta dominates the byte delta.
pub const COMPRESSION_LEVEL: u8 = 1;

/// Size of the streaming compressor's output buffer. Picked at 16 KiB as a
/// balance between (a) per-call drain overhead — too small means many
/// `compress()` invocations to make progress — and (b) memory footprint.
/// The streaming model means total output bytes are unbounded by this;
/// drains happen in 16 KiB chunks regardless of the final body size.
const STREAMING_OUT_BUF_SIZE: usize = 16 * 1024;

/// Allocate a zero-initialized `T` directly on the heap via `allocator`.
///
/// Avoids a 250 KB stack copy that `Box::new_in(T::default(), allocator)`
/// would do for the deflate scratch types.
///
/// # Safety
/// Caller must guarantee that an all-zero bit pattern is a valid value of T.
/// All three deflate scratch types (`HuffmanOxide`, `LocalBuf`, `HashBuffers`)
/// satisfy this: each is a record of fixed-size arrays of integer types whose
/// `Default::default()` is the all-zero pattern.
pub unsafe fn boxed_zeroed_in<T, A: Allocator>(allocator: A) -> Box<T, A> {
    Box::<T, A>::new_zeroed_in(allocator).assume_init()
}

/// Streaming deflate sink. Implements `WriteBytes` so the existing body
/// emitters (`apply_storage_diffs_pubdata`, `LogsStorage::apply_pubdata`)
/// write to it as if it were the raw pubdata destination. Internally each
/// write is fed to miniz incrementally and the emitted compressed bytes are
/// forwarded to both real sinks (`pubdata_dst` for DA commitment,
/// `result_keeper.pubdata` for sequencer / test capture).
///
/// The caller owns the three scratch boxes (`HuffmanOxide`, `LocalBuf`,
/// `HashBuffers`) and constructs the `CompressorOxideInner` borrowing them;
/// `DeflateSink::new` takes ownership of the compressor and the destination
/// references. `finish` must be called to flush trailing bytes — leaving the
/// sink un-flushed produces a truncated deflate stream.
pub struct DeflateSink<'a, A, DST, RK>
where
    A: Allocator,
    DST: WriteBytes + ?Sized,
    RK: IOResultKeeper<EthereumIOTypesConfig>,
{
    compressor: CompressorOxideInner<'a>,
    out_buf: Box<[u8], A>,
    dst: &'a mut DST,
    rk: &'a mut RK,
}

impl<'a, A, DST, RK> DeflateSink<'a, A, DST, RK>
where
    A: Allocator,
    DST: WriteBytes + ?Sized,
    RK: IOResultKeeper<EthereumIOTypesConfig>,
{
    pub fn new(
        compressor: CompressorOxideInner<'a>,
        allocator: A,
        dst: &'a mut DST,
        rk: &'a mut RK,
    ) -> Self {
        // SAFETY: `[u8]` of any length is valid as all zeros.
        let out_buf = unsafe {
            Box::<[u8], A>::new_zeroed_slice_in(STREAMING_OUT_BUF_SIZE, allocator).assume_init()
        };
        Self {
            compressor,
            out_buf,
            dst,
            rk,
        }
    }

    /// Flush trailing compressor state and emit the deflate stream end
    /// marker. Consumes the sink — any further write would corrupt the
    /// stream.
    pub fn finish(mut self) {
        loop {
            let (status, _bytes_in, bytes_out) = compress(
                &mut self.compressor,
                &[],
                &mut self.out_buf,
                TDEFLFlush::Finish,
            );
            if bytes_out > 0 {
                self.dst.write(&self.out_buf[..bytes_out]);
                self.rk.pubdata(&self.out_buf[..bytes_out]);
            }
            match status {
                TDEFLStatus::Done => break,
                TDEFLStatus::Okay => continue,
                TDEFLStatus::BadParam => {
                    panic!("deflate finish returned BadParam — compressor flags are wrong")
                }
                TDEFLStatus::PutBufFailed => {
                    // Only reachable via the callback-style API; the slice
                    // API we use cannot trigger this.
                    panic!("deflate finish returned PutBufFailed")
                }
            }
        }
    }
}

impl<A, DST, RK> WriteBytes for DeflateSink<'_, A, DST, RK>
where
    A: Allocator,
    DST: WriteBytes + ?Sized,
    RK: IOResultKeeper<EthereumIOTypesConfig>,
{
    fn write(&mut self, buf: &[u8]) {
        let mut remaining = buf;
        while !remaining.is_empty() {
            let (status, bytes_in, bytes_out) = compress(
                &mut self.compressor,
                remaining,
                &mut self.out_buf,
                TDEFLFlush::None,
            );
            if bytes_out > 0 {
                self.dst.write(&self.out_buf[..bytes_out]);
                self.rk.pubdata(&self.out_buf[..bytes_out]);
            }
            // miniz with `TDEFLFlush::None` and a drained output buffer
            // always makes progress (either consumes input or fills the
            // output buffer). A no-progress return is an internal bug.
            if bytes_in == 0 && bytes_out == 0 {
                match status {
                    TDEFLStatus::BadParam => {
                        panic!("deflate stream returned BadParam — compressor flags are wrong")
                    }
                    TDEFLStatus::PutBufFailed => panic!("deflate stream returned PutBufFailed"),
                    TDEFLStatus::Okay | TDEFLStatus::Done => {
                        panic!("deflate streaming compressor made no progress")
                    }
                }
            }
            remaining = &remaining[bytes_in..];
        }
    }
}

/// Convenience: produce the compressor flags `DeflateSink` uses.
pub fn streaming_compressor_flags() -> u32 {
    compress_flags_default(COMPRESSION_LEVEL)
}

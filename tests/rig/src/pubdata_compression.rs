//! Host-side measurement helpers for compressing a captured pubdata blob.
//!
//! Used to evaluate the savings of running deflate (and optionally zlib
//! wrapping) over the v2 pubdata layout. No protocol behavior change — the
//! STF still emits the uncompressed blob; this helper only re-runs the bytes
//! through the no_std-friendly `miniz_nostd_compression` deflate so we can
//! report the compressed size alongside the uncompressed one.

use miniz_nostd_compression::deflate::core::CompressorOxideInner;
use miniz_nostd_compression::deflate::{
    compress_flags_default, compress_to_buffer, HashBuffers, HuffmanOxide, LocalBuf,
};

/// Result of running a single compression pass on a captured pubdata blob.
#[derive(Debug, Clone, Copy)]
pub struct CompressionMeasurement {
    pub uncompressed_len: usize,
    pub compressed_len: usize,
}

impl CompressionMeasurement {
    pub fn ratio(&self) -> f64 {
        if self.uncompressed_len == 0 {
            return 1.0;
        }
        self.compressed_len as f64 / self.uncompressed_len as f64
    }

    pub fn savings_bytes(&self) -> i64 {
        self.uncompressed_len as i64 - self.compressed_len as i64
    }
}

/// Run deflate (RFC 1951 raw) on `input` and return how many bytes it produced.
///
/// `level` is a miniz/zlib compression level 0..=10:
/// - 0 = store uncompressed
/// - 1 = fastest
/// - 6 = default
/// - 9 = best compression
/// - 10 = even more checks, very slow
pub fn deflate(input: &[u8], level: u8) -> Vec<u8> {
    deflate_with_flags(input, compress_flags_default(level))
}

/// Run deflate with a zlib wrapper (RFC 1950: 2-byte zlib header + adler-32
/// trailer). This is the format produced by `flate2`'s zlib mode and
/// browsers' `Content-Encoding: deflate`.
pub fn deflate_zlib(input: &[u8], level: u8) -> Vec<u8> {
    use miniz_nostd_compression::deflate::compress_flags_zlib;
    deflate_with_flags(input, compress_flags_zlib(level))
}

fn deflate_with_flags(input: &[u8], flags: u32) -> Vec<u8> {
    let mut huff = Box::new(HuffmanOxide::default());
    let mut local_buf = Box::new(LocalBuf::default());
    let mut hb = Box::new(HashBuffers::default());
    let mut compressor = CompressorOxideInner::new(flags, &mut huff, &mut local_buf, &mut hb);

    // zlib's `deflateBound` worst case is `n + (n>>12) + (n>>14) + (n>>25) + 13`.
    // Use a generous 2x + 128 to stay well above that and accommodate small
    // inputs where the static-block overhead dominates.
    let cap = input.len().saturating_mul(2).max(128) + 128;
    let mut output = vec![0u8; cap];

    let used = compress_to_buffer(&mut compressor, input, &mut output)
        .expect("deflate output buffer underestimated");
    output.truncate(used);
    output
}

/// Convenience: measure raw deflate at the requested level.
pub fn measure_deflate(input: &[u8], level: u8) -> CompressionMeasurement {
    CompressionMeasurement {
        uncompressed_len: input.len(),
        compressed_len: deflate(input, level).len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_handled() {
        let m = measure_deflate(&[], 9);
        assert_eq!(m.uncompressed_len, 0);
        // Deflate always emits at least an empty static block (a few bytes).
        assert!(m.compressed_len <= 16);
    }

    #[test]
    fn repetitive_input_compresses_well() {
        let input = vec![0u8; 4096];
        let m = measure_deflate(&input, 9);
        // 4 KiB of zeros should deflate to well under 1% of its original size.
        assert!(
            m.compressed_len < 64,
            "expected zeros to compress to < 64 bytes, got {}",
            m.compressed_len
        );
    }

    #[test]
    fn zlib_wrapping_is_a_few_bytes_more() {
        let input = vec![0u8; 4096];
        let raw = deflate(&input, 9);
        let zlib = deflate_zlib(&input, 9);
        // Zlib header (2B) + adler-32 trailer (4B) ⇒ ~6 byte difference.
        assert!(
            zlib.len() >= raw.len() + 4 && zlib.len() <= raw.len() + 16,
            "raw={} zlib={}",
            raw.len(),
            zlib.len()
        );
    }
}

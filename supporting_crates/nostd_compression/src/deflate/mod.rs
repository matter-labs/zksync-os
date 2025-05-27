// Based on https://github.com/Frommi/miniz_oxide/blob/f177ab233c9e38cec03f7b35b9e642246c893312/miniz_oxide/src/deflate/mod.rs

mod buffer;
pub mod core;
pub mod stream;

use self::core::*;

pub use self::buffer::{HashBuffers, LocalBuf};
pub use self::core::HuffmanOxide;

/// How much processing the compressor should do to compress the data.
/// `NoCompression` and `Bestspeed` have special meanings, the other levels determine the number
/// of checks for matches in the hash chains and whether to use lazy or greedy parsing.
#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CompressionLevel {
    /// Don't do any compression, only output uncompressed blocks.
    NoCompression = 0,
    /// Fast compression. Uses a special compression routine that is optimized for speed.
    BestSpeed = 1,
    /// Slow/high compression. Do a lot of checks to try to find good matches.
    BestCompression = 9,
    /// Even more checks, can be very slow.
    UberCompression = 10,
    /// Default compromise between speed and compression.
    DefaultLevel = 6,
    /// Use the default compression level.
    DefaultCompression = -1,
}

pub fn compress_flags_default(level: u8) -> u32 {
    create_comp_flags_from_zip_params(level.into(), 0, 0)
}

pub fn compress_flags_zlib(level: u8) -> u32 {
    create_comp_flags_from_zip_params(level.into(), 1, 0)
}

/// Caller is responsible to provide a buffer that is large enough for simplicity.
/// Returns actually filler number of bytes in the output buffer. Will error if the buffer
/// is not large enough
pub fn compress_to_buffer(
    compressor: &mut CompressorOxideInner<'_>,
    mut input: &[u8],
    output: &mut [u8],
) -> Result<usize, ()> {
    let mut out_pos = 0;
    let out_len = output.len();
    loop {
        let (status, bytes_in, bytes_out) = compress(
            compressor,
            input,
            &mut output[out_pos..],
            TDEFLFlush::Finish,
        );
        out_pos += bytes_out;

        match status {
            TDEFLStatus::Done => {
                break;
            }
            TDEFLStatus::Okay if bytes_in <= input.len() => {
                input = &input[bytes_in..];

                // We need more space, so resize the vector.
                if out_len.saturating_sub(out_pos) < 30 {
                    return Err(());
                }
            }
            // Not supposed to happen unless there is a bug.
            _ => panic!("Bug! Unexpectedly failed to compress!"),
        }
    }

    Ok(out_pos)
}

#[cfg(test)]
mod test {
    extern crate alloc;
    use crate::deflate::*;
    use miniz_oxide::inflate::decompress_to_vec;

    /// Compress the input data to a vector, using the specified compression level (0-10).
    fn compress_to_vec(input: &[u8], level: u8) -> Vec<u8> {
        compress_to_vec_inner(input, level, 0, 0)
    }

    /// Simple function to compress data to a vec.
    fn compress_to_vec_inner(input: &[u8], level: u8, window_bits: i32, strategy: i32) -> Vec<u8> {
        // The comp flags function sets the zlib flag if the window_bits parameter is > 0.
        let flags = create_comp_flags_from_zip_params(level.into(), window_bits, strategy);
        let mut huff = Box::new(HuffmanOxide::default());
        let mut local_buf = Box::new(LocalBuf::default());
        let mut b = Box::new(HashBuffers::default());
        let mut d = CompressorOxideInner::new(flags, &mut huff, &mut local_buf, &mut b);
        let mut output = vec![0; input.len() * 4];

        let used_len = compress_to_buffer(&mut d, input, &mut output).unwrap();
        output.truncate(used_len);

        output
    }

    /// Test deflate example.
    ///
    /// Check if the encoder produces the same code as the example given by Mark Adler here:
    /// https://stackoverflow.com/questions/17398931/deflate-encoding-with-static-huffman-codes/17415203
    #[test]
    fn compress_small() {
        let test_data = b"Deflate late";
        let check = [
            0x73, 0x49, 0x4d, 0xcb, 0x49, 0x2c, 0x49, 0x55, 0x00, 0x11, 0x00,
        ];

        let res = compress_to_vec(test_data, 1);
        assert_eq!(&check[..], res.as_slice());

        let res = compress_to_vec(test_data, 9);
        assert_eq!(&check[..], res.as_slice());
    }

    #[test]
    fn compress_huff_only() {
        let test_data = b"Deflate late";

        let res = compress_to_vec_inner(test_data, 1, 0, CompressionStrategy::HuffmanOnly as i32);
        let d = decompress_to_vec(res.as_slice()).expect("Failed to decompress!");
        assert_eq!(test_data, d.as_slice());
    }

    /// Test that a raw block compresses fine.
    #[test]
    fn compress_raw() {
        let text = b"Hello, zlib!";
        let encoded = {
            let len = text.len();
            let notlen = !len;
            let mut encoded = vec![
                1,
                len as u8,
                (len >> 8) as u8,
                notlen as u8,
                (notlen >> 8) as u8,
            ];
            encoded.extend_from_slice(&text[..]);
            encoded
        };

        let res = compress_to_vec(text, 0);
        assert_eq!(encoded, res.as_slice());
    }

    #[test]
    fn short() {
        let test_data = [10, 10, 10, 10, 10, 55];
        let c = compress_to_vec(&test_data, 9);

        let d = decompress_to_vec(c.as_slice()).expect("Failed to decompress!");
        assert_eq!(&test_data, d.as_slice());
        // Check that a static block is used here, rather than a raw block
        // , so the data is actually compressed.
        // (The optimal compressed length would be 5, but neither miniz nor zlib manages that either
        // as neither checks matches against the byte at index 0.)
        assert!(c.len() <= 6);
    }
}

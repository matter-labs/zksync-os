//! Low-level RLP primitives: encoding-length estimation and writing.
//!
//! RLP lists are prefixed with the byte length of their concatenated payload, so
//! a streaming encoder must know each element's encoded length before it can emit
//! the prefix. The `estimate_*` helpers are pure `usize` math; the `apply_*`
//! helpers write the encoding into any `WriteBytes` sink (a hasher for the
//! block/receipt roots, an MPT-leaf buffer in Ethereum mode, ...), so the same
//! primitives serve the receipt encoder and the streaming header / EIP-7702
//! authorization hashes.

use zk_ee::utils::write_bytes::WriteBytes;

// --- encoding-length estimation ---

/// Estimates the length of the RLP encoding of a big-endian number, i.e. the
/// value with its leading zero bytes stripped.
pub fn estimate_number_encoding_len(value: &[u8]) -> usize {
    let first_non_zero_byte = value
        .iter()
        .position(|&byte| byte != 0)
        .unwrap_or(value.len());
    estimate_bytes_encoding_len(&value[first_non_zero_byte..])
}

/// Estimates the length of the RLP encoding of a byte string.
pub fn estimate_bytes_encoding_len(value: &[u8]) -> usize {
    if value.len() == 1 && value[0] < 128 {
        return 1;
    }

    estimate_length_encoding_len(value.len()) + value.len()
}

/// Estimates the length of the RLP length prefix for content of `length` bytes.
///
/// **Must not be used for a single byte less than 128**, which is encoded as
/// itself with no prefix (this function would over-count it by one).
///
/// Please note that this is an internal method, used by `estimate_bytes_encoding_len`,
/// which special cases low single byte slice. There is a public alias
/// `estimate_list_length_encoding_len` to make it clear that it's only for lists,
/// for slices user of this module should use `estimate_bytes_encoding_len`.
///
const fn estimate_length_encoding_len(length: usize) -> usize {
    if length <= 55 {
        1
    } else {
        1 + core::mem::size_of::<usize>() - (length.leading_zeros() / 8) as usize
    }
}

/// Estimates the length of the RLP length prefix for list with content of `length` bytes.
pub const fn estimate_list_length_encoding_len(length: usize) -> usize {
    estimate_length_encoding_len(length)
}

// --- writing into a `WriteBytes` sink ---

/// Applies an RLP length prefix. `OFFSET` is `0x80` for byte strings, `0xc0` for
/// lists. Must not be used for a single byte less than 128.
fn apply_length_encoding<const OFFSET: u8, B: ?Sized + WriteBytes>(length: usize, buffer: &mut B) {
    if length <= 55 {
        buffer.write_byte(OFFSET + length as u8);
    } else {
        let length_bytes = length.to_be_bytes();
        let non_zero_byte = length_bytes.iter().position(|&byte| byte != 0).unwrap();
        buffer.write_byte(OFFSET + 55 + (length_bytes.len() - non_zero_byte) as u8);
        buffer.write(&length_bytes[non_zero_byte..]);
    }
}

/// Applies the RLP length prefix for a byte string of `length` bytes.
///
/// Internal helper for [`apply_bytes_encoding`]; callers that also write the data
/// should use [`apply_bytes_encoding`], which additionally applies the
/// single-byte (`< 0x80`) rule.
fn apply_slice_length_encoding<B: ?Sized + WriteBytes>(length: usize, buffer: &mut B) {
    apply_length_encoding::<0x80, B>(length, buffer)
}

/// Applies the RLP length prefix for a list with `length` payload bytes.
pub fn apply_list_length_encoding<B: ?Sized + WriteBytes>(length: usize, buffer: &mut B) {
    apply_length_encoding::<0xc0, B>(length, buffer)
}

/// Applies the RLP encoding of a byte string (a single byte less than 128
/// encodes as itself, with no length prefix).
pub fn apply_bytes_encoding<B: ?Sized + WriteBytes>(value: &[u8], buffer: &mut B) {
    if value.len() == 1 && value[0] < 128 {
        buffer.write(value);
        return;
    }

    apply_slice_length_encoding(value.len(), buffer);
    buffer.write(value);
}

/// Applies the RLP encoding of a big-endian number, i.e. the value with its
/// leading zero bytes stripped (`0` encodes as the empty string `0x80`).
pub fn apply_number_encoding<B: ?Sized + WriteBytes>(value: &[u8], buffer: &mut B) {
    let first_non_zero_byte = value
        .iter()
        .position(|&byte| byte != 0)
        .unwrap_or(value.len());
    apply_bytes_encoding(&value[first_non_zero_byte..], buffer);
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    struct VecSink(Vec<u8>);
    impl WriteBytes for VecSink {
        fn write(&mut self, buf: &[u8]) {
            self.0.extend_from_slice(buf);
        }
    }

    fn applied<F: FnOnce(&mut VecSink)>(f: F) -> Vec<u8> {
        let mut sink = VecSink(Vec::new());
        f(&mut sink);
        sink.0
    }

    #[test]
    fn number_encoding_known_answers() {
        // (input bytes, expected RLP). Numbers are big-endian with leading zeros stripped.
        let cases: &[(&[u8], &[u8])] = &[
            (&[], &[0x80]),           // empty -> empty string
            (&[0x00], &[0x80]),       // zero -> empty string
            (&[0x00, 0x00], &[0x80]), // multi-byte zero -> empty string
            (&[0x05], &[0x05]),       // single byte < 0x80 -> itself
            (&[0x7f], &[0x7f]),       // boundary: 0x7f -> itself
            (&[0x80], &[0x81, 0x80]), // 0x80 needs a length prefix
            (&[0xff], &[0x81, 0xff]),
            (&[0x01, 0x00], &[0x82, 0x01, 0x00]),       // 256
            (&[0x00, 0x12, 0x34], &[0x82, 0x12, 0x34]), // leading zero stripped
        ];
        for (input, expected) in cases {
            assert_eq!(
                &applied(|s| apply_number_encoding(input, s)),
                expected,
                "input {input:?}"
            );
            // The estimate must match the number of bytes actually written.
            assert_eq!(
                estimate_number_encoding_len(input),
                expected.len(),
                "estimate {input:?}"
            );
        }
    }

    #[test]
    fn bytes_encoding_known_answers() {
        let cases: &[(&[u8], Vec<u8>)] = &[
            (&[], alloc::vec![0x80]),     // empty string
            (&[0x00], alloc::vec![0x00]), // single byte < 0x80 -> itself (NOT stripped)
            (&[0x7f], alloc::vec![0x7f]),
            (&[0x80], alloc::vec![0x81, 0x80]), // single byte >= 0x80 -> length-prefixed
            (&[0xff], alloc::vec![0x81, 0xff]),
            (&[0x01, 0x02], alloc::vec![0x82, 0x01, 0x02]),
        ];
        for (input, expected) in cases {
            assert_eq!(
                &applied(|s| apply_bytes_encoding(input, s)),
                expected,
                "input {input:?}"
            );
            assert_eq!(
                estimate_bytes_encoding_len(input),
                expected.len(),
                "estimate {input:?}"
            );
        }
    }

    #[test]
    fn bytes_encoding_length_prefix_boundary() {
        // 55-byte string: short form, single prefix byte 0x80 + 55 = 0xb7.
        let s55 = alloc::vec![0xaau8; 55];
        let out = applied(|s| apply_bytes_encoding(&s55, s));
        assert_eq!(out[0], 0xb7);
        assert_eq!(out.len(), 1 + 55);
        assert_eq!(estimate_bytes_encoding_len(&s55), out.len());

        // 56-byte string: long form, 0xb8 then one length byte (56).
        let s56 = alloc::vec![0xaau8; 56];
        let out = applied(|s| apply_bytes_encoding(&s56, s));
        assert_eq!(&out[..2], &[0xb8, 56]);
        assert_eq!(out.len(), 2 + 56);
        assert_eq!(estimate_bytes_encoding_len(&s56), out.len());

        // 256-byte string: 0xb9 then two length bytes (0x01, 0x00).
        let s256 = alloc::vec![0xaau8; 256];
        let out = applied(|s| apply_bytes_encoding(&s256, s));
        assert_eq!(&out[..3], &[0xb9, 0x01, 0x00]);
        assert_eq!(out.len(), 3 + 256);
        assert_eq!(estimate_bytes_encoding_len(&s256), out.len());
    }

    #[test]
    fn list_length_prefix_known_answers() {
        // (payload length, expected list prefix bytes)
        let cases: &[(usize, &[u8])] = &[
            (0, &[0xc0]), // empty list
            (1, &[0xc1]),
            (55, &[0xf7]),     // short-list boundary 0xc0 + 55
            (56, &[0xf8, 56]), // long list: 0xc0 + 55 + 1, then length
            (255, &[0xf8, 0xff]),
            (256, &[0xf9, 0x01, 0x00]),
        ];
        for (len, expected) in cases {
            assert_eq!(
                &applied(|s| apply_list_length_encoding(*len, s)),
                expected,
                "len {len}"
            );
            // Prefix length only (payload excluded).
            assert_eq!(
                estimate_list_length_encoding_len(*len),
                expected.len(),
                "estimate {len}"
            );
        }
    }
}

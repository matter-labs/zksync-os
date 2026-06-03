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

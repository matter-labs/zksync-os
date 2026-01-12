//!
//! The rlp encoding implementation for hashing.
//! It writes the rlp encoded values directly to hasher without allocating additional memory.
//!
//! There are also methods to estimate the encoding length, useful for lists encoding.
//! The list encoding pipeline should look like:
//! - Estimate encoding length for every list element.
//! - Calculate the list encoding length.
//! - Apply the list encoding length encoding.
//! - Apply encoded elements.
//!

use crypto::sha3::Digest;

/// Addresses are encoded as 20 bytes
pub const ADDRESS_ENCODING_LEN: usize = 21;

// methods for the encoding length estimation

///
/// Estimates length of the number rlp encoding.
///
pub fn estimate_number_encoding_len(value: &[u8]) -> usize {
    let first_non_zero_byte = value
        .iter()
        .position(|&byte| byte != 0)
        .unwrap_or(value.len());
    estimate_bytes_encoding_len(&value[first_non_zero_byte..])
}

///
/// Estimates extra length of encoding length of some payload
///
pub const fn estimate_encoding_len_for_payload_length(payload_encoding_len: usize) -> usize {
    if payload_encoding_len <= 55 {
        1
    } else {
        1 + core::mem::size_of::<usize>() - (payload_encoding_len.leading_zeros() / 8) as usize
    }
}

///
/// Estimates length of the bytes rlp encoding.
///
pub fn estimate_bytes_encoding_len(value: &[u8]) -> usize {
    if value.len() == 1 && value[0] < 128 {
        return 1;
    }

    estimate_length_encoding_len(value.len()) + value.len()
}

///
/// Estimates length of the bytes(or list) length rlp encoding.
///
/// **Note that it shouldn't be used for a single byte less than 128.**
///
pub fn estimate_length_encoding_len(length: usize) -> usize {
    if length < 56 {
        1
    } else {
        let length_bytes = length.to_be_bytes();
        let non_zero_byte = length_bytes.iter().position(|&byte| byte != 0).unwrap();
        1 + length_bytes.len() - non_zero_byte
    }
}

// methods to apply the encoding to the hasher

///
/// Applies the number rlp encoding to the hasher.
///
pub fn apply_number_encoding_to_hash(value: &[u8], hasher: &mut impl Digest) {
    // if the value is 0, then it should be encoded as empty bytes
    let first_non_zero_byte = value
        .iter()
        .position(|&byte| byte != 0)
        .unwrap_or(value.len());
    apply_bytes_encoding_to_hash(&value[first_non_zero_byte..], hasher);
}

///
/// Applies the bytes rlp encoding to the hasher.
///
pub fn apply_bytes_encoding_to_hash(value: &[u8], hasher: &mut impl Digest) {
    if value.len() == 1 && value[0] < 128 {
        hasher.update(value);
        return;
    }

    apply_length_encoding_to_hash(value.len(), 128, hasher);
    hasher.update(value);
}

///
/// Applies the list rlp encoding to the hasher.
///
pub fn apply_list_length_encoding_to_hash(length: usize, hasher: &mut impl Digest) {
    apply_length_encoding_to_hash(length, 192, hasher);
}

///
/// Applies the length rlp encoding to the hasher.
/// offset = 128 should be used for bytes, 192 - for list.
///
/// Note that it shouldn't be used for a single byte less than 128.
///
fn apply_length_encoding_to_hash(length: usize, offset: u8, hasher: &mut impl Digest) {
    if length < 56 {
        hasher.update(&[offset + length as u8])
    } else {
        let length_bytes = length.to_be_bytes();
        let non_zero_byte = length_bytes.iter().position(|&byte| byte != 0).unwrap();
        hasher.update(&[offset + 55 + (length_bytes.len() - non_zero_byte) as u8]);
        hasher.update(&length_bytes[non_zero_byte..]);
    }
}

use crypto::MiniDigest;

pub mod minimal_rlp_parser;

#[cfg(test)]
pub mod test_helpers;
#[cfg(test)]
mod tests;

pub(crate) fn u64_encoding_len(value: u64) -> usize {
    if value < 0x80 {
        1
    } else {
        let bits = 64 - value.leading_zeros();
        let encoding_bytes = (bits.next_multiple_of(8) / 8) as usize;
        1 + encoding_bytes
    }
}

pub(crate) fn apply_u64_encoding_to_hash(value: u64, hasher: &mut impl MiniDigest) {
    if value == 0 {
        hasher.update(&[0x80]);
    } else if value < 0x80 {
        hasher.update(&[value as u8]);
    } else {
        let bits = 64 - value.leading_zeros();
        let encoding_bytes = (bits.next_multiple_of(8) / 8) as usize;
        let length_bytes = value.to_be_bytes();
        hasher.update(&[0x80 + encoding_bytes as u8]);
        hasher.update(&length_bytes[(8 - encoding_bytes)..]);
    }
}

pub(crate) fn apply_list_concatenation_encoding_to_hash(length: u32, hasher: &mut impl MiniDigest) {
    if length < 56 {
        hasher.update(&[0xc0 + length as u8]);
    } else {
        let bits = 32 - length.leading_zeros();
        let encoding_bytes = (bits.next_multiple_of(8) / 8) as usize;
        let length_bytes = length.to_be_bytes();
        hasher.update(&[0xc0 + 55 + encoding_bytes as u8]);
        hasher.update(&length_bytes[(4 - encoding_bytes)..]);
    }
}

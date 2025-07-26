use super::*;

pub(crate) fn list_encoding_prefix_len(list_concatenation_len: usize) -> usize {
    if list_concatenation_len <= 55 {
        1
    } else if list_concatenation_len < 1 << 8 {
        2
    } else if list_concatenation_len < 1 << 16 {
        3
    } else {
        unreachable!()
    }
}

pub(crate) fn encode_list_len_into_buffer(
    buffer: &mut impl ByteBuffer,
    list_concatenation_len: usize,
) {
    if list_concatenation_len <= 55 {
        buffer.write_byte(0xc0 + (list_concatenation_len as u8));
    } else if list_concatenation_len < 1 << 8 {
        buffer.write_slice(&[0xf8, list_concatenation_len as u8]);
    } else if list_concatenation_len < 1 << 16 {
        buffer.write_slice(&[
            0xf9,
            (list_concatenation_len >> 8) as u8,
            list_concatenation_len as u8,
        ]);
    } else {
        unreachable!()
    }
}

use super::*;

pub(crate) fn rlp_parse_short_bytes<'a>(src: &'a [u8]) -> Result<&'a [u8], ()> {
    let mut data = src;
    let b0 = consume(&mut data, 1)?;
    let bb0 = b0[0];
    if bb0 >= 0xc0 {
        // it can not be a list
        return Err(());
    }
    if bb0 < 0x80 {
        if src.len() != 1 {
            return Err(());
        }
        Ok(src)
    } else if bb0 < 0xb8 {
        let expected_len = (bb0 - 0x80) as usize;
        if data.len() != expected_len {
            return Err(());
        }
        Ok(data)
    } else {
        Err(())
    }
}

pub(crate) fn slice_encoding_len(slice: &[u8]) -> usize {
    #[allow(clippy::if_same_then_else)]
    if slice.len() == 0 {
        1
    } else if slice.len() == 1 && slice[0] < 0x80 {
        1
    } else if slice.len() <= 55 {
        1 + slice.len()
    } else if slice.len() < 1 << 8 {
        2 + slice.len()
    } else if slice.len() < 1 << 16 {
        3 + slice.len()
    } else if slice.len() < 1 << 24 {
        4 + slice.len()
    } else {
        unreachable!()
    }
}

pub(crate) fn encode_slice_into_buffer(slice: &[u8], buffer: &mut impl ByteBuffer) {
    if slice.len() == 0 {
        buffer.write_byte(0x80);
    } else if slice.len() == 1 && slice[0] < 0x80 {
        if slice[0] < 0x80 {
            buffer.write_byte(slice[0]);
        }
    } else if slice.len() <= 55 {
        buffer.write_byte(0x80 + (slice.len() as u8));
        buffer.write_slice(slice);
    } else if slice.len() < 1 << 8 {
        buffer.write_slice(&[0xb7 + 1, slice.len() as u8]);
        buffer.write_slice(slice);
    } else if slice.len() < 1 << 16 {
        buffer.write_slice(&[0xb7 + 2, (slice.len() >> 8) as u8, slice.len() as u8]);
        buffer.write_slice(slice);
    } else if slice.len() < 1 << 24 {
        buffer.write_slice(&[
            0xb7 + 3,
            (slice.len() >> 16) as u8,
            (slice.len() >> 8) as u8,
            slice.len() as u8,
        ]);
        buffer.write_slice(slice);
    } else {
        unreachable!()
    }
}

pub(crate) fn list_encoding_prefix_len(list_concatenation_len: usize) -> usize {
    if list_concatenation_len <= 55 {
        1
    } else if list_concatenation_len < 1 << 8 {
        2
    } else if list_concatenation_len < 1 << 16 {
        3
    } else if list_concatenation_len < 1 << 24 {
        4
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
    } else if list_concatenation_len < 1 << 24 {
        buffer.write_slice(&[
            0xfa,
            (list_concatenation_len >> 16) as u8,
            (list_concatenation_len >> 8) as u8,
            list_concatenation_len as u8,
        ]);
    } else {
        unreachable!()
    }
}

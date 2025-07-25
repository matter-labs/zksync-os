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

// pub(crate) fn slice_encoding_prefix_len(slice: &[u8]) -> usize {
//     if slice.len() == 1 && slice[0] < 0x80 {
//         0
//     } else {
//         if slice.len() <= 55 {
//             1
//         } else {
//             if slice.len() < 256 {
//                 2
//             } else if slice.len() < 1 << 16 {
//                 3
//             } else {
//                 unreachable!()
//             }
//         }
//     }
// }

// pub(crate) fn encode_slice_into_buffer(buffer: &mut impl ByteBuffer, slice: &[u8]) {
//     if slice.len() == 1 && slice[0] < 0x80 {
//         buffer.write_byte(slice[0]);
//     } else {
//         if slice.len() <= 55 {
//             buffer.write_byte(0x80 + (slice.len() as u8));
//             buffer.write_slice(slice);
//         } else {
//             todo!();
//         }
//     }
// }

// pub(crate) fn encode_large_slice_len_into_buffer(buffer: &mut impl ByteBuffer, slice_len: usize) {
//     assert!(slice_len >= 32);
//     if slice_len <= 55 {
//         buffer.write_byte(0x80 + (slice_len as u8));
//     } else {
//         if slice_len < 1 << 8 {
//             buffer.write_slice(&[0xb8, slice_len as u8]);
//         } else if slice_len < 1 << 16 {
//             buffer.write_slice(&[0xb9, (slice_len >> 8) as u8, slice_len as u8]);
//         } else {
//             unreachable!()
//         }
//     }
// }

// Nibbles is a rare example of owning structure, that should solve a problem of non-inclusion proofs.
// In the ideal case we can take a full "path" that we request a proof for, and use it for all the nodes.
// Unfortunately there are cases when the node that is given by the proof has a different path,
// and we will have to make a duplicate and own it

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Nibbles {
    prefix_bits: u32,
    segment_bits: u32,
    underlying: [u64; 4],
}

fn shl_assign(data: &mut [u64; 4], rhs: u32) {
    if rhs == 0 {
        return;
    }

    let (limbs, bits) = (rhs / 64, rhs % 64);

    match limbs {
        0 => {
            if bits != 0 {
                let mut carry = data[0] >> (64 - bits);
                data[0] <<= bits;
                let t = data[1] >> (64 - bits);
                data[1] = data[1] << bits | carry;
                carry = t;
                let t = data[2] >> (64 - bits);
                data[2] = data[2] << bits | carry;
                carry = t;
                data[3] = data[3] << bits | carry;
            }
        }
        1 => {
            // let compiler optimize
            data[3] = data[2];
            data[2] = data[1];
            data[1] = data[0];
            data[0] = 0;

            if bits != 0 {
                let mut carry = data[1] >> (64 - bits);
                data[1] <<= bits;
                let t = data[2] >> (64 - bits);
                data[2] = data[2] << bits | carry;
                carry = t;
                data[3] = data[3] << bits | carry;
            }
        }
        2 => {
            data[3] = data[1];
            data[2] = data[0];
            data[1] = 0;
            data[0] = 0;

            if bits != 0 {
                let carry = data[2] >> (64 - bits);
                data[2] <<= bits;
                data[3] = data[3] << bits | carry;
            }
        }
        3 => {
            data[3] = data[0];
            data[0] = 0;
            data[1] = 0;
            data[2] = 0;

            data[3] <<= bits;
        }
        _ => {
            *data = [0u64; 4];
        }
    }
}

impl Nibbles {
    fn new_from_full_path(encoding: &[u8]) -> Self {
        assert_eq!(encoding.len(), 32);
        let segment_bits = (encoding.len() * 8) as u32;
        let mut result = [0u64; 4];
        let mut array_chunks = encoding.array_chunks::<8>();
        let mut num_filled = 0;
        for src in &mut array_chunks {
            result[num_filled] = u64::from_le_bytes(*src);
            num_filled += 1;
        }
        let remaining = array_chunks.remainder();
        if remaining.len() > 0 {
            let mut buffer = [0u8; 8];
            buffer[..remaining.len()].copy_from_slice(remaining);
            result[num_filled] = u64::from_le_bytes(buffer);
        }

        Self {
            underlying: result,
            prefix_bits: 0,
            segment_bits,
        }
    }

    pub(crate) fn new_from_node_encoding(node_encoding: &[u8]) -> Self {
        assert!(node_encoding.len() <= 33);
        let b = node_encoding[0];
        let shift_amount = if (b >> 4) & 1 == 1 {
            4u32
        } else {
            debug_assert_eq!(b & 0x0f, 0);

            8u32
        };
        let segment_bits = (node_encoding.len() * 8) as u32 - shift_amount;
        // then decode in LE manner
        let mut result = [0u64; 4];
        let mut array_chunks = node_encoding.array_chunks::<8>();
        let mut num_filled = 0;
        for src in &mut array_chunks {
            result[num_filled] = u64::from_le_bytes(*src);
            num_filled += 1;
        }
        let remaining = array_chunks.remainder();
        if remaining.len() > 0 {
            if num_filled == 3 {
                assert_eq!(node_encoding.len(), 33);
                assert_eq!(remaining.len(), 1);
                assert_eq!(shift_amount, 8);
                // shift first
                shl_assign(&mut result, shift_amount);
                // set last element
                result[3] |= (remaining[0] as u64) >> 56;
            } else {
                let mut buffer = [0u8; 8];
                buffer[..remaining.len()].copy_from_slice(remaining);
                result[num_filled] = u64::from_le_bytes(buffer);
                shl_assign(&mut result, shift_amount);
            }
        }

        Self {
            underlying: result,
            prefix_bits: 0,
            segment_bits,
        }
    }
}


// // NOTE: Path and PathSegment do NOT have special rules about equality, but
// // we will take care on constructing them to ensure that we will zero-out unused underlyings is needed
// // in PathSegment

// #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// pub(crate) struct Path {
//     nibbles: Nibbles,
// }

// impl Path {
//     pub(crate) fn new(path: &[u8]) -> Self {
//         assert_eq!(path.len(), 32);
//         Self {
//             nibbles: Nibbles::new_from_full_path(path),
//         }
//     }

//     pub(crate) fn is_empty(&self) -> bool {
//         self.nibbles.segment_bits - self.nibbles.prefix_bits == 0
//     }

//     pub(crate) const fn prefix_len(&self) -> u32 {
//         self.nibbles.prefix_bits / 8
//     }

//     pub(crate) fn into_prefix_only(&self) -> PathSegment {
//         todo!();
//     }

//     #[inline]
//     pub(crate) fn follow(
//         &mut self,
//         segment: Nibbles,
//         skip_single_char: bool,
//     ) -> Result<PathSegment, ()> {
//         debug_assert_eq!(segment.prefix_bits, 0);
//         if self.
//         // raw nibbles are bytes, that have to be interpreted as chars
//         let mut num_nibbles = raw_nibbles.len() * 2 - 1;
//         if skip_single_char == false {
//             num_nibbles -= 1;
//         }
//         if self.remaining_path().len() < num_nibbles {
//             return Err(());
//         }
//         let taken_nibbles = &self.remaining_path()[..num_nibbles];
//         dbg!(std::str::from_utf8(taken_nibbles).unwrap());
//         dbg!(hex::encode(raw_nibbles));
//         // actually check char by char
//         let mut it = raw_nibbles.iter();

//         unsafe {
//             let mut nibbles_byte = *it.next().unwrap_unchecked();
//             let mut process_next = false;
//             if skip_single_char == false {
//                 process_next = true;
//             }
//             for el in taken_nibbles.iter() {
//                 let value = if process_next {
//                     nibbles_byte = *it.next().unwrap_unchecked();
//                     process_next = false;
//                     nibbles_byte >> 4
//                 } else {
//                     process_next = true;
//                     nibbles_byte & 0x0f
//                 };
//                 let path_digit = Self::path_char_to_digit(*el);
//                 if path_digit != value {
//                     dbg!(std::str::from_utf8(&[*el]).unwrap());
//                     dbg!(path_digit);
//                     dbg!(value);
//                     return Err(());
//                 }
//             }
//         }
//         let segment = PathSegment {
//             path: *self,
//             segment_len: num_nibbles,
//         };
//         self.prefix_len += num_nibbles;

//         Ok(segment)
//     }

//     pub(crate) fn take_branch(&mut self) -> Result<usize, ()> {
//         if self.remaining_path().is_empty() {
//             return Err(());
//         }
//         let t = Self::path_char_to_digit(self.remaining_path()[0]);
//         self.prefix_len += 1;

//         Ok(t as usize)
//     }
// }


// #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// pub(crate) struct PathSegment {
//     nibbles: Nibbles,
// }

// impl PathSegment {
//     pub(crate) fn is_empty(&self) -> bool {
//         self.segment().is_empty()
//     }

//     pub(crate) const fn prefix_len(&self) -> usize {
//         self.path.prefix_len()
//     }

//     pub(crate) fn segment(&self) -> &'a [u8] {
//         &self.path.remaining_path()[..self.segment_len]
//     }

//     pub(crate) fn prefix(&self) -> &'a [u8] {
//         self.path.prefix()
//     }
// }
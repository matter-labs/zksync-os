mod nodes;
mod trie;
// mod nibbles;
mod parse_node;
mod rlp;
// mod updates;
mod preimages;

use core::alloc::Allocator;
use core::mem::MaybeUninit;
use crypto::MiniDigest;
use zk_ee::utils::Bytes32;

pub(crate) use self::nodes::*;
pub(crate) use self::parse_node::*;
pub(crate) use self::rlp::*;
pub(crate) use self::trie::*;

pub use self::preimages::*;
pub use self::trie::EthereumMPT;

pub const EMPTY_ROOT_HASH: Bytes32 = Bytes32::from_hex("39bef1777deb3dfb14f64b9f81ced092c501fee72f90e93d03bb95ee89df9837");

#[cfg(test)]
mod tests;

pub(crate) const EMPTY_LIST_ENCODING: &'static [u8] = &[0x80];

pub(crate) fn path_char_to_digit(c: u8) -> u8 {
    // c
    match c {
        b'A'..=b'F' => c - b'A' + 10,
        b'a'..=b'f' => c - b'a' + 10,
        b'0'..=b'9' => c - b'0',
        _ => {
            unreachable!()
        }
    }
}

#[inline]
pub(crate) fn consume<'a>(src: &mut &'a [u8], bytes: usize) -> Result<&'a [u8], ()> {
    let (data, rest) = src.split_at_checked(bytes).ok_or(())?;
    *src = rest;

    Ok(data)
}

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

pub trait ByteBuffer {
    fn write_byte(&mut self, byte: u8);
    fn write_slice(&mut self, slice: &[u8]);
}

pub trait WordBuffer {
    fn write_word(&mut self, word: usize);
    fn write_slice(&mut self, slice: &[usize]);
}

impl<T: MiniDigest> ByteBuffer for T {
    fn write_byte(&mut self, byte: u8) {
        self.update(&[byte]);
    }
    fn write_slice(&mut self, slice: &[u8]) {
        self.update(slice);
    }
}

pub trait InterningBuffer<'a>: ByteBuffer {
    fn flush(self) -> &'a [u8];
}

pub trait InterningWordBuffer<'a>: WordBuffer {
    fn flush(self) -> &'a [usize];
    fn flush_as_bytes(self, byte_len: usize) -> &'a [u8];
}

impl WordBuffer for () {
    fn write_word(&mut self, _word: usize) {
        unreachable!()
    }
    fn write_slice(&mut self, _slice: &[usize]) {
        unreachable!()
    }
}

impl<'a> InterningWordBuffer<'a> for () {
    fn flush(self) -> &'a [usize] {
        unreachable!()
    }
    fn flush_as_bytes(self, _byte_len: usize) -> &'a [u8] {
        unreachable!()
    }
}

pub trait Interner<'a>: 'a {
    const SUPPORTS_WORD_LEVEL_INTERNING: bool;

    type Buffer: InterningBuffer<'a>
    where
        Self: 'a;
    type WordBuffer: InterningWordBuffer<'a>
    where
        Self: 'a;
    fn get_buffer(&'_ mut self, capacity: usize) -> Result<Self::Buffer, ()>;
    fn get_word_buffer(&'_ mut self, word_capacity: usize) -> Result<Self::WordBuffer, ()>;
}

pub struct MaybeUninitByteBuffer<'a> {
    buffer: &'a mut [MaybeUninit<u8>],
    num_written: usize,
}

impl<'a> ByteBuffer for MaybeUninitByteBuffer<'a> {
    fn write_byte(&mut self, byte: u8) {
        self.buffer[self.num_written].write(byte);
        self.num_written += 1;
    }
    fn write_slice(&mut self, slice: &[u8]) {
        self.buffer[self.num_written..][..slice.len()].write_copy_of_slice(slice);
        self.num_written += slice.len();
    }
}

impl<'a> InterningBuffer<'a> for MaybeUninitByteBuffer<'a> {
    fn flush(self) -> &'a [u8] {
        unsafe { core::slice::from_raw_parts(self.buffer.as_ptr().cast(), self.num_written) }
    }
}

pub struct MaybeUninitWordBuffer<'a> {
    buffer: &'a mut [MaybeUninit<usize>],
    num_written: usize,
}

impl<'a> WordBuffer for MaybeUninitWordBuffer<'a> {
    fn write_word(&mut self, word: usize) {
        self.buffer[self.num_written].write(word);
        self.num_written += 1;
    }
    fn write_slice(&mut self, slice: &[usize]) {
        self.buffer[self.num_written..][..slice.len()].write_copy_of_slice(slice);
        self.num_written += slice.len();
    }
}

impl<'a> InterningWordBuffer<'a> for MaybeUninitWordBuffer<'a> {
    fn flush_as_bytes(self, byte_len: usize) -> &'a [u8] {
        assert!(byte_len <= self.num_written * core::mem::size_of::<usize>());
        unsafe { core::slice::from_raw_parts(self.buffer.as_ptr().cast(), byte_len) }
    }

    fn flush(self) -> &'a [usize] {
        unsafe { core::slice::from_raw_parts(self.buffer.as_ptr().cast(), self.num_written) }
    }
}

pub struct BoxInterner<A: Allocator> {
    buffer: Box<[MaybeUninit<usize>], A>,
    used: usize,
}

impl<A: Allocator> BoxInterner<A> {
    pub fn with_capacity_in(byte_capacity: usize, allocator: A) -> Self {
        let word_capacity = byte_capacity.next_multiple_of(core::mem::size_of::<usize>())
            / core::mem::size_of::<usize>();
        Self {
            buffer: Box::new_uninit_slice_in(word_capacity, allocator),
            used: 0,
        }
    }
}

impl<'a, A: Allocator + 'a> Interner<'a> for BoxInterner<A> {
    const SUPPORTS_WORD_LEVEL_INTERNING: bool = true;

    type Buffer
        = MaybeUninitByteBuffer<'a>
    where
        Self: 'a;

    type WordBuffer
        = MaybeUninitWordBuffer<'a>
    where
        Self: 'a;

    fn get_buffer(&'_ mut self, capacity: usize) -> Result<Self::Buffer, ()>
    where
        A: 'a,
    {
        let next_multiple = capacity.next_multiple_of(core::mem::size_of::<usize>());
        let word_capacity = next_multiple
            / core::mem::size_of::<usize>();
        if self.used + word_capacity > self.buffer.len() {
            return Err(());
        }
        unsafe {
            let to_use = core::slice::from_raw_parts_mut(
                self.buffer.as_mut_ptr().add(self.used).cast(),
                next_multiple,
            );
            self.used += word_capacity;

            Ok(MaybeUninitByteBuffer {
                buffer: to_use,
                num_written: 0,
            })
        }
    }

    fn get_word_buffer(&'_ mut self, word_capacity: usize) -> Result<Self::WordBuffer, ()> {
        if self.used + word_capacity > self.buffer.len() {
            return Err(());
        }
        unsafe {
            let to_use = core::slice::from_raw_parts_mut(
                self.buffer.as_mut_ptr().add(self.used),
                word_capacity,
            );
            self.used += word_capacity;

            Ok(MaybeUninitWordBuffer {
                buffer: to_use,
                num_written: 0,
            })
        }
    }
}

// Some generic convenience function
pub trait InternerExt<'a>: Interner<'a> {
    fn intern_nibbles(&'_ mut self, nibbles_encoding: &'_ [u8]) -> Result<(&'a [u8], bool), ()> {
        if nibbles_encoding.len() < 1 {
            return Err(());
        }
        let t = nibbles_encoding[0] >> 4;
        let mut skip_single_char = true;
        let is_leaf = if t == 0 || t == 1 {
            if t == 0 {
                if nibbles_encoding[0] & 0x0f != 0 {
                    return Err(());
                }
                skip_single_char = false;
            }
            false
        } else if t == 2 || t == 3 {
            if t == 2 {
                if nibbles_encoding[0] & 0x0f != 0 {
                    return Err(());
                }
                skip_single_char = false;
            }
            true
        } else {
            return Err(());
        };

        let mut num_nibbles = nibbles_encoding.len() * 2 - 1;
        if skip_single_char == false {
            num_nibbles -= 1;
        }

        let mut buffer = self.get_buffer(num_nibbles)?;
        let mut it = nibbles_encoding.iter();
        unsafe {
            let mut nibbles_byte = *it.next().unwrap_unchecked();
            let mut process_next = false;
            if skip_single_char == false {
                process_next = true;
            }
            for _ in 0..num_nibbles {
                let value = if process_next {
                    nibbles_byte = *it.next().unwrap_unchecked();
                    process_next = false;
                    nibbles_byte >> 4
                } else {
                    process_next = true;
                    nibbles_byte & 0x0f
                };
                buffer.write_byte(value);
            }
        }
        let path_segment = buffer.flush();

        Ok((path_segment, is_leaf))
    }

    fn update_leaf_value<D: MiniDigest>(
        &mut self,
        existing_leaf_node: &LeafNode<'_>,
        new_raw_value: &[u8],
        hasher: &mut D,
    ) -> Result<&'a [u8], ()>
    where
        D::HashOutput: AsRef<[u8]>,
    {
        // we need to make an RLP of the leaf and intern a new key (we are not interested in value actually)

        let mut total_list_concatenated_len = existing_leaf_node.raw_nibbles_encoding.len();
        total_list_concatenated_len += new_raw_value.len();
        let total_len =
            total_list_concatenated_len + list_encoding_prefix_len(total_list_concatenated_len);

        if total_len < 32 {
            // we need RLP of RLP
            let mut buffer = self.get_buffer(1 + total_len)?;
            let writer = &mut buffer;
            // we need to RLP it on top - it is short
            writer.write_byte(0x80 + (total_len as u8));

            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            // now encode two elements, by taking their raw encodings
            writer.write_slice(existing_leaf_node.raw_nibbles_encoding);
            writer.write_slice(new_raw_value);
            let result = buffer.flush();
            dbg!(hex::encode(result));

            Ok(result)
        } else {
            // {
            //     let mut buffer = self.get_buffer(total_len)?;
            //     let writer = &mut buffer;

            //     encode_list_len_into_buffer(writer, total_list_concatenated_len);
            //     // now encode two elements, by taking their raw encodings
            //     writer.write_slice(existing_leaf_node.raw_nibbles_encoding);
            //     writer.write_slice(new_raw_value);
            //     let result = buffer.flush();
            //     dbg!(hex::encode(result));
            // }

            // we need to do the same into hasher
            let writer = hasher;
            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            // now encode two elements
            writer.update(existing_leaf_node.raw_nibbles_encoding);
            writer.update(new_raw_value);
            let key = writer.finalize_reset();

            let mut buffer = self.get_buffer(33)?;
            buffer.write_byte(0x80 + 32);
            buffer.write_slice(key.as_ref());

            Ok(buffer.flush())
        }
    }

    fn update_branch_node<D: MiniDigest>(
        &mut self,
        existing_branch_node: &mut BranchNode<'a>,
        branch_index: usize,
        new_raw_value: &[u8],
        hasher: &mut D,
    ) -> Result<&'a [u8], ()>
    where
        D::HashOutput: AsRef<[u8]>,
    {
        let mut total_list_concatenated_len =
            existing_branch_node.branches_encodings_concatenation.len();
        total_list_concatenated_len -=
            existing_branch_node.child_encoding_lengths[branch_index] as usize;
        total_list_concatenated_len += new_raw_value.len();
        // and empty value
        total_list_concatenated_len += 1;

        let total_len =
            total_list_concatenated_len + list_encoding_prefix_len(total_list_concatenated_len);

        if total_len < 32 {
            // we need RLP of RLP
            let mut buffer = self.get_buffer(1 + total_len)?;
            let writer = &mut buffer;

            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            let encoding_offset = list_encoding_prefix_len(total_list_concatenated_len);
            let mut new_encoding_len = 0usize;
            let mut raw_encoding_slice = existing_branch_node.branches_encodings_concatenation;
            for idx in 0..16 {
                let len = existing_branch_node.child_encoding_lengths[idx] as usize;
                let (child, rest) = raw_encoding_slice.split_at(len);
                raw_encoding_slice = rest;
                if branch_index == idx {
                    existing_branch_node.child_encoding_lengths[idx] = new_raw_value.len() as u8;
                    writer.write_slice(new_raw_value);
                    new_encoding_len += new_raw_value.len();
                } else {
                    writer.write_slice(child);
                    new_encoding_len += child.len();
                }
            }
            // empty value
            writer.write_byte(0x80);
            let result = buffer.flush();

            // update encoding part using interned buffer
            existing_branch_node.branches_encodings_concatenation =
                &result[encoding_offset..][..new_encoding_len];

            Ok(result)
        } else {
            // {
            //     let mut buffer = self.get_buffer(3 + total_len)?;
            //     let writer = &mut buffer;

            //     encode_list_len_into_buffer(writer, total_list_concatenated_len);
            //     let mut raw_encoding_slice = existing_branch_node.branches_encodings_concatenation;
            //     for idx in 0..16 {
            //         let len = existing_branch_node.child_encoding_lengths[idx] as usize;
            //         let (child, rest) = raw_encoding_slice.split_at(len);
            //         raw_encoding_slice = rest;
            //         if branch_index == idx {
            //             writer.write_slice(new_raw_value);
            //         } else {
            //             writer.write_slice(child);
            //         }
            //     }
            //     writer.write_byte(0x80);
            //     let result = buffer.flush();
            //     dbg!(hex::encode(result));
            // }

            // Here we actually have to do double-interning, and update both concatenation,
            // lengths of individual leaf encodings, and compute a key

            {
                // we only need a buffer that is as long as concatenation of branches
                let mut buffer = self.get_buffer(total_list_concatenated_len)?;
                let writer = &mut buffer;
                let mut raw_encoding_slice = existing_branch_node.branches_encodings_concatenation;
                for idx in 0..16 {
                    let len = existing_branch_node.child_encoding_lengths[idx] as usize;
                    let (child, rest) = raw_encoding_slice.split_at(len);
                    raw_encoding_slice = rest;
                    if branch_index == idx {
                        existing_branch_node.child_encoding_lengths[idx] =
                            new_raw_value.len() as u8;
                        writer.write_slice(new_raw_value);
                    } else {
                        writer.write_slice(child);
                    }
                }
                let result = buffer.flush();
                existing_branch_node.branches_encodings_concatenation = result;
            }

            let writer = hasher;
            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            // we updated it above
            writer.write_slice(existing_branch_node.branches_encodings_concatenation);
            // empty value
            writer.write_byte(0x80);
            let key = writer.finalize_reset();

            let mut buffer = self.get_buffer(33)?;
            buffer.write_byte(0x80 + 32);
            buffer.write_slice(key.as_ref());

            Ok(buffer.flush())
        }
    }

    fn update_extension_value<D: MiniDigest>(
        &mut self,
        existing_extension_node: &ExtensionNode<'_>,
        new_raw_value: &[u8],
        hasher: &mut D,
    ) -> Result<&'a [u8], ()>
    where
        D::HashOutput: AsRef<[u8]>,
    {
        // we need to make an RLP of the leaf and intern a new key (we are not interested in value actually)

        let mut total_list_concatenated_len = existing_extension_node.raw_nibbles_encoding.len();
        total_list_concatenated_len += new_raw_value.len();
        let total_len =
            total_list_concatenated_len + list_encoding_prefix_len(total_list_concatenated_len);

        if total_len < 32 {
            // we need RLP of RLP
            let mut buffer = self.get_buffer(1 + total_len)?;
            let writer = &mut buffer;
            // we need to RLP it on top - it is short
            writer.write_byte(0x80 + (total_len as u8));

            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            // now encode two elements, by taking their raw encodings
            writer.write_slice(existing_extension_node.raw_nibbles_encoding);
            writer.write_slice(new_raw_value);
            let result = buffer.flush();

            Ok(result)
        } else {
            {
                let mut buffer = self.get_buffer(total_len)?;
                let writer = &mut buffer;

                encode_list_len_into_buffer(writer, total_list_concatenated_len);
                // now encode two elements, by taking their raw encodings
                writer.write_slice(existing_extension_node.raw_nibbles_encoding);
                writer.write_slice(new_raw_value);
                let result = buffer.flush();
                dbg!(hex::encode(result));
            }

            // we need to do the same into hasher
            let writer = hasher;
            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            // now encode two elements
            writer.update(existing_extension_node.raw_nibbles_encoding);
            writer.update(new_raw_value);
            let key = writer.finalize_reset();

            let mut buffer = self.get_buffer(33)?;
            buffer.write_byte(0x80 + 32);
            buffer.write_slice(key.as_ref());

            Ok(buffer.flush())
        }
    }

    fn convert_branch_value_into_leaf<D: MiniDigest>(
        &mut self,
        branch_index: usize,
        raw_value: &[u8],
        hasher: &mut D,
    ) -> Result<&'a [u8], ()>
    where
        D::HashOutput: AsRef<[u8]>,
    {
        // we need to make an RLP of the leaf and intern a new key (we are not interested in value actually)

        // nibbles encoding is always short in this case - single byte
        let nibbles = [0x30 + (branch_index as u8)];
        let mut total_list_concatenated_len = 1;
        total_list_concatenated_len += raw_value.len();
        let total_len =
            total_list_concatenated_len + list_encoding_prefix_len(total_list_concatenated_len);

        if total_len < 32 {
            // we need RLP of RLP
            let mut buffer = self.get_buffer(1 + total_len)?;
            let writer = &mut buffer;
            // we need to RLP it on top - it is short
            writer.write_byte(0x80 + (total_len as u8));

            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            // now encode two elements, by taking their raw encodings
            writer.write_slice(&nibbles);
            writer.write_slice(raw_value);
            let result = buffer.flush();
            dbg!(hex::encode(result));

            Ok(result)
        } else {
            // {
            //     let mut buffer = self.get_buffer(total_len)?;
            //     let writer = &mut buffer;

            //     encode_list_len_into_buffer(writer, total_list_concatenated_len);
            //     // now encode two elements, by taking their raw encodings
            //     writer.write_slice(existing_leaf_node.raw_nibbles_encoding);
            //     writer.write_slice(new_raw_value);
            //     let result = buffer.flush();
            //     dbg!(hex::encode(result));
            // }

            // we need to do the same into hasher
            let writer = hasher;
            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            // now encode two elements
            writer.update(&nibbles);
            writer.update(raw_value);
            let key = writer.finalize_reset();

            let mut buffer = self.get_buffer(33)?;
            buffer.write_byte(0x80 + 32);
            buffer.write_slice(key.as_ref());

            Ok(buffer.flush())
        }
    }
}

// Default impl
impl<'a, T: Interner<'a>> InternerExt<'a> for T {}

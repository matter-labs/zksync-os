mod nodes;
mod trie;
// mod nibbles;
mod updates;
mod rlp;

use core::alloc::Allocator;
use core::mem::MaybeUninit;

use crypto::MiniDigest;

pub(crate) use self::rlp::*;

pub use self::trie::EthereumMPT;

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

#[cfg(test)]
mod tests;

pub(crate) const EMPTY_LIST_ENCODING: &'static [u8] = &[0x80];

pub(crate) use self::nodes::*;

pub trait ByteBuffer {
    fn write_byte(&mut self, byte: u8);
    fn write_slice(&mut self, slice: &[u8]);
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

pub trait Interner<'a>: 'a {
    type Buffer: InterningBuffer<'a>
    where
        Self: 'a;
    fn get_buffer(&'_ mut self, capacity: usize) -> Result<Self::Buffer, ()>;
}

pub struct MaybeUninitBuffer<'a> {
    buffer: &'a mut [MaybeUninit<u8>],
    num_written: usize,
}

impl<'a> ByteBuffer for MaybeUninitBuffer<'a> {
    fn write_byte(&mut self, byte: u8) {
        self.buffer[self.num_written].write(byte);
        self.num_written += 1;
    }
    fn write_slice(&mut self, slice: &[u8]) {
        self.buffer[self.num_written..][..slice.len()].write_copy_of_slice(slice);
        self.num_written += slice.len();
    }
}

impl<'a> InterningBuffer<'a> for MaybeUninitBuffer<'a> {
    fn flush(self) -> &'a [u8] {
        unsafe { core::slice::from_raw_parts(self.buffer.as_ptr().cast(), self.num_written) }
    }
}

pub struct BoxInterner<A: Allocator> {
    buffer: Box<[MaybeUninit<u8>], A>,
    used: usize,
}

impl<A: Allocator> BoxInterner<A> {
    pub fn with_capacity_in(capacity: usize, allocator: A) -> Self {
        Self {
            buffer: Box::new_uninit_slice_in(capacity, allocator),
            used: 0,
        }
    }
}

impl<'a, A: Allocator + 'a> Interner<'a> for BoxInterner<A> {
    type Buffer
        = MaybeUninitBuffer<'a>
    where
        Self: 'a;

    fn get_buffer(&'_ mut self, capacity: usize) -> Result<Self::Buffer, ()>
    where
        A: 'a,
    {
        if self.used + capacity > self.buffer.len() {
            return Err(());
        }
        unsafe {
            let to_use =
                core::slice::from_raw_parts_mut(self.buffer.as_mut_ptr().add(self.used), capacity);
            self.used += capacity;

            Ok(MaybeUninitBuffer {
                buffer: to_use,
                num_written: 0,
            })
        }
    }
}

// Some generic convenience function
pub trait InternerExt<'a>: Interner<'a> {
    fn intern_nibbles(
        &'_ mut self,
        nibbles_encoding: &'_ [u8],
    ) -> Result<(&'a [u8], bool), ()> {
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
    ) -> Result<&'a [u8], ()> where D::HashOutput: AsRef<[u8]>{
        // we need to make an RLP of the leaf and intern a new key (we are not interested in value actually)

        let mut total_list_concatenated_len = existing_leaf_node.raw_nibbles_encoding.len();
        total_list_concatenated_len += new_raw_value.len();
        let total_len = total_list_concatenated_len + list_encoding_prefix_len(total_list_concatenated_len);

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
    ) -> Result<&'a [u8], ()> where D::HashOutput: AsRef<[u8]>{
        let mut total_list_concatenated_len = existing_branch_node.branches_encodings_concatenation.len();
        total_list_concatenated_len -= existing_branch_node.child_encoding_lengths[branch_index] as usize;
        total_list_concatenated_len += new_raw_value.len();
        // and empty value
        total_list_concatenated_len += 1;

        let total_len = total_list_concatenated_len + list_encoding_prefix_len(total_list_concatenated_len);

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
            existing_branch_node.branches_encodings_concatenation = &result[encoding_offset..][..new_encoding_len];

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
                        existing_branch_node.child_encoding_lengths[idx] = new_raw_value.len() as u8;
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
    ) -> Result<&'a [u8], ()> where D::HashOutput: AsRef<[u8]>{
        // we need to make an RLP of the leaf and intern a new key (we are not interested in value actually)

        let mut total_list_concatenated_len = existing_extension_node.raw_nibbles_encoding.len();
        total_list_concatenated_len += new_raw_value.len();
        let total_len = total_list_concatenated_len + list_encoding_prefix_len(total_list_concatenated_len);

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
    ) -> Result<&'a [u8], ()> where D::HashOutput: AsRef<[u8]>{
        // we need to make an RLP of the leaf and intern a new key (we are not interested in value actually)

        // nibbles encoding is always short in this case - single byte
        let nibbles = [0x30 + (branch_index as u8)];
        let mut total_list_concatenated_len = 1;
        total_list_concatenated_len += raw_value.len();
        let total_len = total_list_concatenated_len + list_encoding_prefix_len(total_list_concatenated_len);

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
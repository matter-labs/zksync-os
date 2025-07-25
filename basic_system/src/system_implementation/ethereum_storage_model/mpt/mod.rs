mod nodes;
mod parse_node;
mod preimages;
mod rlp;
mod trie;
mod updates;

use alloc::boxed::Box;
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

pub(crate) const EMPTY_SLICE_ENCODING: &[u8] = &[0x80];

// Hash of RLP encoded empty slice
pub const EMPTY_ROOT_HASH: Bytes32 =
    Bytes32::from_hex("56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421");

#[cfg(test)]
mod tests;

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
    fn flush_mut(self) -> &'a mut [u8];
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

    fn flush_mut(self) -> &'a mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(self.buffer.as_mut_ptr().cast(), self.num_written)
        }
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
        let word_capacity = next_multiple / core::mem::size_of::<usize>();
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
pub trait ETHMPTInternerExt<'a>: Interner<'a> {
    fn intern_slice(&'_ mut self, slice: &'_ [u8]) -> Result<&'a [u8], ()> {
        let mut buffer = self.get_buffer(slice.len())?;
        buffer.write_slice(slice);

        Ok(buffer.flush())
    }

    fn intern_slice_mut(&'_ mut self, slice: &'_ [u8]) -> Result<&'a mut [u8], ()> {
        let mut buffer = self.get_buffer(slice.len())?;
        buffer.write_slice(slice);

        Ok(buffer.flush_mut())
    }

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

    // will return key
    fn make_leaf_key(
        &mut self,
        path_for_nibbles: &[u8],
        pre_encoded_value: &[u8],
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<&'a [u8], ()> {
        debug_assert!(path_for_nibbles.len() > 0);
        // we need to make an RLP of the leaf and intern a new key (we are not interested in value actually)
        let num_nibbles = path_for_nibbles.len();
        let num_bytes_to_encode_nibbles = if num_nibbles % 2 == 1 {
            (num_nibbles + 1) / 2
        } else {
            (num_nibbles / 2) + 1
        };
        debug_assert!(num_bytes_to_encode_nibbles >= 1);
        let rlp_prefix_len = if num_nibbles == 1 {
            // only possible values are 0x1X, so it's always byte itself
            0
        } else {
            // max length is 17 bytes, so 1 byte
            1
        };
        let nibbles_encoding_len = num_bytes_to_encode_nibbles + rlp_prefix_len;
        let mut total_list_concatenated_len = nibbles_encoding_len;
        total_list_concatenated_len += pre_encoded_value.len();
        // total_list_concatenated_len += slice_encoding_prefix_len(pre_encoded_value);
        let total_len =
            total_list_concatenated_len + list_encoding_prefix_len(total_list_concatenated_len);

        if total_len < 32 {
            let mut buffer = self.get_buffer(1 + total_len)?;
            let writer = &mut buffer;
            // we need to RLP it on top - it is short
            writer.write_byte(0x80 + (total_len as u8));

            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            if rlp_prefix_len > 0 {
                writer.write_byte(0x80 + (num_bytes_to_encode_nibbles as u8));
            }
            write_nibbles(writer, true, path_for_nibbles);
            writer.write_slice(pre_encoded_value);
            let result = buffer.flush();

            Ok(result)
        } else {
            let writer = hasher;
            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            if rlp_prefix_len > 0 {
                writer.write_byte(0x80 + (num_bytes_to_encode_nibbles as u8));
            }
            write_nibbles(writer, true, path_for_nibbles);
            writer.write_slice(pre_encoded_value);
            // encode_slice_into_buffer(writer, pre_encoded_value);
            let key = writer.finalize_reset();

            let mut buffer = self.get_buffer(33)?;
            buffer.write_byte(0x80 + 32);
            buffer.write_slice(key.as_ref());

            Ok(buffer.flush())
        }
    }

    // will return key
    fn make_extension_key(
        &mut self,
        path_for_nibbles: &[u8],
        maybe_preencoded_nibbles: &[u8],
        pre_encoded_value: &[u8],
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<&'a [u8], ()> {
        // we will ignore pre-encoded nibbles, and only assert basic consistency
        debug_assert!(path_for_nibbles.len() > 0);
        // we need to make an RLP of the leaf and intern a new key (we are not interested in value actually)
        let num_nibbles = path_for_nibbles.len();
        let num_bytes_to_encode_nibbles = if num_nibbles % 2 == 1 {
            (num_nibbles + 1) / 2
        } else {
            (num_nibbles / 2) + 1
        };
        debug_assert!(num_bytes_to_encode_nibbles >= 1);
        let rlp_prefix_len = if num_nibbles == 1 {
            // possible values are 0x3X, so it's always byte itself
            0
        } else {
            // max length is 17 bytes, so 1 byte
            1
        };
        let nibbles_encoding_len = num_bytes_to_encode_nibbles + rlp_prefix_len;
        if maybe_preencoded_nibbles.len() > 0 {
            assert_eq!(maybe_preencoded_nibbles.len(), nibbles_encoding_len);
        }
        let mut total_list_concatenated_len = nibbles_encoding_len;
        total_list_concatenated_len += pre_encoded_value.len();
        let total_len =
            total_list_concatenated_len + list_encoding_prefix_len(total_list_concatenated_len);

        if total_len < 32 {
            // we need RLP of RLP
            let mut buffer = self.get_buffer(1 + total_len)?;
            let writer = &mut buffer;
            // we need to RLP it on top - it is short
            writer.write_byte(0x80 + (total_len as u8));

            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            if rlp_prefix_len > 0 {
                writer.write_byte(0x80 + (num_bytes_to_encode_nibbles as u8));
            }
            write_nibbles(writer, false, path_for_nibbles);
            writer.write_slice(pre_encoded_value);
            let result = buffer.flush();

            Ok(result)
        } else {
            let writer = hasher;
            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            if rlp_prefix_len > 0 {
                writer.write_byte(0x80 + (num_bytes_to_encode_nibbles as u8));
            }
            write_nibbles(writer, false, path_for_nibbles);
            writer.write_slice(pre_encoded_value);
            let key = writer.finalize_reset();

            let mut buffer = self.get_buffer(33)?;
            buffer.write_byte(0x80 + 32);
            buffer.write_slice(key.as_ref());

            Ok(buffer.flush())
        }
    }

    fn make_branch_key(
        &mut self,
        child_keys: &[&'_ [u8]; 16],
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<&'a [u8], ()> {
        let mut total_list_concatenated_len = 0usize;
        for child_key in child_keys.iter() {
            total_list_concatenated_len += child_key.len();
        }
        // and empty value
        total_list_concatenated_len += 1;

        let total_len =
            total_list_concatenated_len + list_encoding_prefix_len(total_list_concatenated_len);

        if total_len < 32 {
            // we need RLP of RLP
            let mut buffer = self.get_buffer(1 + total_len)?;
            let writer = &mut buffer;

            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            for child_key in child_keys.iter() {
                writer.write_slice(*child_key);
            }
            // empty value
            writer.write_byte(0x80);
            let result = buffer.flush();

            Ok(result)
        } else {
            let writer = hasher;
            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            // branches
            for child_key in child_keys.iter() {
                writer.write_slice(*child_key);
            }
            // empty value
            writer.write_byte(0x80);
            let key = writer.finalize_reset();

            let mut buffer = self.get_buffer(33)?;
            buffer.write_byte(0x80 + 32);
            buffer.write_slice(key.as_ref());

            Ok(buffer.flush())
        }
    }

    // will return key
    fn make_terminal_branch_value_key(
        &mut self,
        pre_encoded_value: &[u8],
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<&'a [u8], ()> {
        let total_len = pre_encoded_value.len();
        if total_len < 32 {
            let mut buffer = self.get_buffer(total_len)?;
            let writer = &mut buffer;
            writer.write_slice(pre_encoded_value);
            let result = buffer.flush();

            Ok(result)
        } else {
            let writer = hasher;
            writer.write_slice(pre_encoded_value);
            let key = writer.finalize_reset();

            let mut buffer = self.get_buffer(33)?;
            buffer.write_byte(0x80 + 32);
            buffer.write_slice(key.as_ref());

            Ok(buffer.flush())
        }
    }
}

// Default impl
impl<'a, T: Interner<'a>> ETHMPTInternerExt<'a> for T {}

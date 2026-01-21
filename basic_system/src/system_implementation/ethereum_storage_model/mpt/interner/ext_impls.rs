use super::super::*;
use super::*;

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
    fn make_leaf_key_for_value(
        &mut self,
        path_for_nibbles: &[u8],
        mut leaf_value: LeafValue<'_>,
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<&'a [u8], ()> {
        // we need to make an RLP of the leaf and intern a new key (we are not interested in value actually)
        let num_nibbles = path_for_nibbles.len();
        let num_bytes_to_encode_nibbles = if num_nibbles % 2 == 1 {
            (num_nibbles + 1) / 2
        } else {
            (num_nibbles / 2) + 1
        };
        debug_assert!(num_bytes_to_encode_nibbles >= 1);

        let rlp_prefix_len = if num_nibbles <= 1 {
            // only possible values are 0x3X or 0x20
            0
        } else {
            1
        };
        let nibbles_encoding_len = num_bytes_to_encode_nibbles + rlp_prefix_len;
        let mut total_list_concatenated_len = nibbles_encoding_len;
        let leaf_value_rlp_encoding_len = leaf_value.rlp_encoding_length();
        total_list_concatenated_len += leaf_value_rlp_encoding_len;
        let total_len =
            total_list_concatenated_len + list_encoding_prefix_len(total_list_concatenated_len);

        if total_len < 32 {
            let mut buffer = self.get_buffer(total_len)?;
            let writer = &mut buffer;

            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            if rlp_prefix_len > 0 {
                writer.write_byte(0x80 + (num_bytes_to_encode_nibbles as u8));
            }
            write_nibbles(writer, true, path_for_nibbles);
            leaf_value.rlp_encode_into(&mut buffer);
            let result = buffer.flush();

            Ok(result)
        } else {
            // {
            //     let mut leaf_buffer = self.get_buffer(total_len)?;
            //     let writer = &mut leaf_buffer;
            //     encode_list_len_into_buffer(writer, total_list_concatenated_len);
            //     if rlp_prefix_len > 0 {
            //         writer.write_byte(0x80 + (num_bytes_to_encode_nibbles as u8));
            //     }
            //     write_nibbles(writer, true, path_for_nibbles);
            //     leaf_value.rlp_encode_into(writer);
            //     dbg!(hex::encode(leaf_buffer.flush()));
            // }

            let writer = hasher;
            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            if rlp_prefix_len > 0 {
                writer.write_byte(0x80 + (num_bytes_to_encode_nibbles as u8));
            }
            write_nibbles(writer, true, path_for_nibbles);
            leaf_value.rlp_encode_into(writer);
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
            // possible values are 0x3X, so it's always byte itself
            0
        } else {
            // max length is 17 bytes, so 1 byte
            1
        };
        let nibbles_encoding_len = num_bytes_to_encode_nibbles + rlp_prefix_len;
        let mut total_list_concatenated_len = nibbles_encoding_len;
        total_list_concatenated_len += pre_encoded_value.len();
        let total_len =
            total_list_concatenated_len + list_encoding_prefix_len(total_list_concatenated_len);

        if total_len < 32 {
            let mut buffer = self.get_buffer(total_len)?;
            let writer = &mut buffer;

            encode_list_len_into_buffer(writer, total_list_concatenated_len);
            if rlp_prefix_len > 0 {
                writer.write_byte(0x80 + (num_bytes_to_encode_nibbles as u8));
            }
            write_nibbles(writer, false, path_for_nibbles);
            writer.write_slice(pre_encoded_value);
            let result = buffer.flush();

            Ok(result)
        } else {
            // {
            //     let mut extension_buffer = self.get_buffer(total_len)?;
            //     let writer = &mut extension_buffer;
            //     encode_list_len_into_buffer(writer, total_list_concatenated_len);
            //     if rlp_prefix_len > 0 {
            //         writer.write_byte(0x80 + (num_bytes_to_encode_nibbles as u8));
            //     }
            //     write_nibbles(writer, false, path_for_nibbles);
            //     writer.write_slice(pre_encoded_value);
            //     dbg!(hex::encode(extension_buffer.flush()));
            // }

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
            let mut buffer = self.get_buffer(total_len)?;
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
            // {
            //     let mut branch_buffer = self.get_buffer(33 * 17 + 32)?;
            //     let writer = &mut branch_buffer;
            //     encode_list_len_into_buffer(writer, total_list_concatenated_len);
            //     // branches
            //     for child_key in child_keys.iter() {
            //         writer.write_slice(*child_key);
            //     }
            //     // empty value
            //     writer.write_byte(0x80);
            //     dbg!(hex::encode(branch_buffer.flush()));
            // }

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
}

// Default impl
impl<'a, T: Interner<'a>> ETHMPTInternerExt<'a> for T {}

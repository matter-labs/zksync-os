use super::*;

// NOTE: we can consider this slice as potentially "lazy" for storage purposes, so it's possible to avoid
// asking caller to make RLP slice envelope on top of whatever is encoded inside (opaque bytes)

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RLPSlice<'a> {
    full_encoding: &'a [u8],
    raw_data_offset: usize,
}

impl<'a> RLPSlice<'a> {
    pub const fn empty() -> Self {
        Self {
            full_encoding: EMPTY_SLICE_ENCODING,
            raw_data_offset: 1,
        }
    }
    pub(crate) const fn full_encoding(&self) -> &'a [u8] {
        self.full_encoding
    }

    pub fn data(&self) -> &'a [u8] {
        // we pre-validated
        unsafe { self.full_encoding.get_unchecked(self.raw_data_offset..) }
    }

    pub const fn is_empty(&self) -> bool {
        self.raw_data_offset == self.full_encoding.len()
    }

    #[track_caller]
    pub fn from_slice(mut data: &'a [u8]) -> Result<Self, ()> {
        let new = Self::parse(&mut data)?;
        if data.is_empty() == false {
            Err(())
        } else {
            Ok(new)
        }
    }

    #[track_caller]
    pub fn parse(data: &mut &'a [u8]) -> Result<Self, ()> {
        let data_start = data.as_ptr();
        let b0 = consume(data, 1)?;
        let bb0 = b0[0];
        let mut raw_data_offset = 1;
        if bb0 >= 0xc0 {
            // it can not be a list
            return Err(());
        }
        if bb0 < 0x80 {
            raw_data_offset -= 1;
            // fallthrough
        } else if bb0 < 0xb8 {
            let expected_len = (bb0 - 0x80) as usize;
            let _ = consume(data, expected_len)?;
        } else if bb0 < 0xc0 {
            let length_encoding_length = (bb0 - 0xb7) as usize;
            let length_encoding_bytes = consume(data, length_encoding_length)?;
            raw_data_offset += length_encoding_length;
            if length_encoding_bytes.len() > 2 {
                return Err(());
            }
            let mut be_bytes = [0u8; 4];
            be_bytes[(4 - length_encoding_bytes.len())..].copy_from_slice(length_encoding_bytes);
            let length = u32::from_be_bytes(be_bytes) as usize;
            let _ = consume(data, length)?;
        } else {
            return Err(());
        }

        Ok(Self {
            full_encoding: unsafe { core::slice::from_ptr_range(data_start..data.as_ptr()) },
            raw_data_offset,
        })
    }
}

pub(crate) enum ParsedNode<'a> {
    Leaf(LeafNode<'a>),
    Extension(ExtensionNode<'a>),
    BranchHint { num_occupied: usize },
}

fn parse_node_piece<'a>(data: &mut &'a [u8]) -> Result<&'a [u8], ()> {
    let data_start = data.as_ptr();
    let b0 = consume(data, 1)?;
    let bb0 = b0[0];
    if bb0 >= 0xc0 {
        // in very rare cases the piece can be the list itself. Then we will return it in full after
        // small validity check. It can not be too long anyway
        let expected_len = (bb0 - 0xc0) as usize;
        let _ = consume(data, expected_len)?;

        return Ok(unsafe { core::slice::from_ptr_range(data_start..data.as_ptr()) });
    }
    if bb0 < 0x80 {
        Ok(unsafe { core::slice::from_ptr_range(data_start..data.as_ptr()) })
    } else if bb0 == 0x80 {
        Ok(&[])
    } else if bb0 < 0xb8 {
        let expected_len = (bb0 - 0x80) as usize;
        let _ = consume(data, expected_len)?;

        Ok(unsafe { core::slice::from_ptr_range(data_start..data.as_ptr()) })
    } else if bb0 < 0xc0 {
        let length_encoding_length = (bb0 - 0xb7) as usize;
        let length_encoding_bytes = consume(data, length_encoding_length)?;
        if length_encoding_bytes.len() > 2 {
            return Err(());
        }
        let mut be_bytes = [0u8; 4];
        be_bytes[(4 - length_encoding_bytes.len())..].copy_from_slice(length_encoding_bytes);
        let length = u32::from_be_bytes(be_bytes) as usize;
        let _ = consume(data, length)?;

        Ok(unsafe { core::slice::from_ptr_range(data_start..data.as_ptr()) })
    } else {
        Err(())
    }
}

#[inline]
fn parse_initial<'a>(raw_encoding: &'a [u8]) -> Result<(usize, [&'a [u8]; 17], usize), ()> {
    if raw_encoding.len() < 3 {
        return Err(());
    }
    let mut data = raw_encoding;
    let b0 = consume(&mut data, 1)?;
    let b0 = b0[0];
    // we can not make any conclusion based on the first byte. At best we can make a decision that it's a list,
    // but not even the number of elements in it...
    if b0 < 0xc0 {
        return Err(());
    }
    if b0 < 0xf8 {
        // list of unknown(!) length, even though the concatenation is short. Yes, we can not make a decision about
        // validity until we parse the full encoding, but at least let's reject some trivial cases
        let expected_len = b0 - 0xc0;
        if data.len() != expected_len as usize {
            return Err(());
        }
        // either it's a leaf/extension that is a list of two, or branch
        let mut pieces = [&[][..]; 17];
        let mut filled = 0;
        let mut num_non_empty_branches = 0;
        for dst in pieces.iter_mut() {
            let piece = parse_node_piece(&mut data)?;
            *dst = piece;
            if dst.is_empty() == false {
                num_non_empty_branches += 1;
            }
            filled += 1;
            if data.is_empty() {
                break;
            }
        }
        if data.is_empty() == false {
            return Err(());
        }

        Ok((filled, pieces, num_non_empty_branches))
    } else {
        // list of large length. But we do not expect it "too large"
        let length_encoding_length = (b0 - 0xf7) as usize;
        let length_encoding_bytes = consume(&mut data, length_encoding_length)?;
        if length_encoding_bytes.len() > 2 {
            return Err(());
        }
        let mut be_bytes = [0u8; 4];
        be_bytes[(4 - length_encoding_bytes.len())..].copy_from_slice(length_encoding_bytes);
        let length = u32::from_be_bytes(be_bytes) as usize;
        if data.len() != length {
            return Err(());
        }
        let mut pieces = [&[][..]; 17];
        let mut filled = 0;
        let mut num_non_empty_branches = 0;
        for dst in pieces.iter_mut() {
            let piece = parse_node_piece(&mut data)?;
            *dst = piece;
            if dst.is_empty() == false {
                num_non_empty_branches += 1;
            }
            filled += 1;
            if data.is_empty() {
                break;
            }
        }
        if data.is_empty() == false {
            return Err(());
        }

        Ok((filled, pieces, num_non_empty_branches))
    }
}

// returns note type hints, list pieces, and
pub(crate) fn parse_node_from_bytes<'a>(
    key: &'a [u8],
    raw_encoding: &'a [u8],
    interner: &'_ mut (impl Interner<'a> + 'a),
) -> Result<(ParsedNode<'a>, [&'a [u8]; 17], &'a [u8]), ()> {
    let (num_filled, pieces, num_non_empty_branches) = parse_initial(raw_encoding)?;

    if num_filled == 2 {
        // leaf or extension
        // nibbles bytes(!) have to be re-interpreted at hex-chars(!), and then matched against the path
        // we reparse a little
        let nibbles_encoding = RLPSlice::from_slice(pieces[0])?;
        let nibbles = nibbles_encoding.data();
        let (path_segment, is_leaf) = interner.intern_nibbles(nibbles)?;
        if is_leaf == false {
            // extension
            let extension_node = ExtensionNode {
                cached_key: key,
                path_segment,
                parent_node: NodeType::unlinked(), // will be re-linked
                child_node: NodeType::unlinked(),  // will be re-linked
            };

            Ok((
                ParsedNode::Extension(extension_node),
                pieces,
                pieces[1], // will parse later on if we will descend
            ))
        } else {
            let value = RLPSlice::from_slice(pieces[1])?;
            let leaf_node = LeafNode {
                cached_key: key,
                path_segment,
                parent_node: NodeType::unlinked(),
                value: LeafValue::RLPEnveloped { envelope: value },
            };

            Ok((ParsedNode::Leaf(leaf_node), pieces, &[]))
        }
    } else if num_filled == 17 {
        // branch
        if pieces[16].is_empty() == false {
            // can not have a value in our applications
            return Err(());
        }
        if num_non_empty_branches < 2 {
            return Err(());
        }

        // it is a branch, and we must parse it in full, but only take a single path that we are interested in. We do not need to
        // verify well-formedness of branches too much, just to the extend that they are short enough

        for branch_value in pieces[..16].iter() {
            if branch_value.len() > 33 {
                return Err(());
            }
        }

        let parsed = ParsedNode::BranchHint {
            num_occupied: num_non_empty_branches,
        };

        Ok((parsed, pieces, &[]))
    } else {
        Err(())
    }
}

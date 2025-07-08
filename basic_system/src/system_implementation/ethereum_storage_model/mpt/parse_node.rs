use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct RLPSlice<'a> {
    full_encoding: &'a [u8],
    raw_data_offset: usize,
}

impl<'a> RLPSlice<'a> {
    pub(crate) const fn empty() -> Self {
        Self {
            full_encoding: EMPTY_SLICE_ENCODING,
            raw_data_offset: 1,
        }
    }
    pub(crate) const fn full_encoding(&self) -> &'a [u8] {
        self.full_encoding
    }

    pub(crate) fn data(&self) -> &'a [u8] {
        // we pre-validated
        unsafe { self.full_encoding.get_unchecked(self.raw_data_offset..) }
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.raw_data_offset == self.full_encoding.len()
    }

    #[track_caller]
    pub(crate) fn parse(data: &mut &'a [u8]) -> Result<Self, ()> {
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
    Branch(BranchNode<'a>),
}

#[inline]
fn parse_initial<'a>(raw_encoding: &'a [u8]) -> Result<(usize, [RLPSlice<'a>; 17], usize), ()> {
    // we try to insert node encoding and see if it exists
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
        let mut pieces = [RLPSlice::empty(); 17];
        let mut filled = 0;
        let mut num_non_empty_branches = 0;
        for dst in pieces.iter_mut() {
            // and itself it must be a string, not a list
            *dst = RLPSlice::parse(&mut data)?;
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
        let mut pieces = [RLPSlice::empty(); 17];
        let mut filled = 0;
        let mut num_non_empty_branches = 0;
        for dst in pieces.iter_mut() {
            // and itself it must be a string, not a list, and can not be longer than 32 bytes
            *dst = RLPSlice::parse(&mut data)?;
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

pub(crate) fn parse_node_from_bytes<'a>(
    raw_encoding: &'a [u8],
    interner: &'_ mut (impl Interner<'a> + 'a),
) -> Result<(ParsedNode<'a>, [RLPSlice<'a>; 17]), ()> {
    let (num_filled, pieces, num_non_empty_branches) = parse_initial(raw_encoding)?;

    if num_filled == 2 {
        // leaf or extension
        // nibbles bytes(!) have to be re-interpreted at hex-chars(!), and then matched against the path
        let nibbles_encoding = pieces[0];
        let nibbles = nibbles_encoding.data();
        let (path_segment, is_leaf) = interner.intern_nibbles(nibbles)?;
        if is_leaf == false {
            // extension
            let extension_node = ExtensionNode {
                path_segment,
                parent_node: NodeType::unlinked(), // will be re-linked
                child_node: NodeType::unlinked(),  // will be re-linked
                raw_nibbles_encoding: nibbles_encoding.full_encoding(),
                next_node_key: pieces[1],
            };

            Ok((ParsedNode::Extension(extension_node), pieces))
        } else {
            let leaf_node = LeafNode {
                // key: key,
                // prefix,
                path_segment,
                raw_nibbles_encoding: nibbles_encoding.full_encoding(),
                parent_node: NodeType::unlinked(),
                // raw_encoding,
                value: pieces[1],
            };

            Ok((ParsedNode::Leaf(leaf_node), pieces))
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

        let child_nodes = [NodeType::unlinked(); 16];
        // let mut child_encoding_lengths = [0u8; 16];
        for branch_value in pieces[..16].iter() {
            if branch_value.data().len() > 32 {
                return Err(());
            }
        }
        // for (idx, branch_value) in pieces[..16].iter().enumerate() {
        //     if branch_value.data().len() > 32 {
        //         return Err(());
        //     }
        //     child_encoding_lengths[idx] = branch_value.full_encoding().len() as u8;
        //     if branch_value.is_empty() {
        //         child_nodes[idx] = NodeType::empty();
        //     }
        // }
        let branch_node = BranchNode {
            // key,
            // prefix: branch_prefix_as_path,
            parent_node: NodeType::unlinked(),
            child_nodes,
            // branches_encodings_concatenation,
            // child_encoding_lengths,
            // raw_encoding,
            _marker: core::marker::PhantomData,
        };

        Ok((ParsedNode::Branch(branch_node), pieces))
    } else {
        return Err(());
    }
}

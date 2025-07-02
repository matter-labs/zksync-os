use crate::system_implementation::ethereum_storage_model::mpt::trie::rlp_parse_short_bytes;

use super::*;

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub(crate) fn delete_from_branch_node<D: MiniDigest>(
        &mut self,
        node: NodeType,
        prefix: &[u8],
        branch_index: usize,
        interner: &'_ mut (impl Interner<'a> + 'a),
        hasher: &mut D,
    ) -> Result<&'a [u8], ()> where D::HashOutput: AsRef<[u8]> {
        // dbg!(hex::encode(pre_encoded_branch_value));
        let (_, (_, existing_node)) = self.branch_nodes.get_persistent_mut_by_index(node.index());
        // two cases - if we need to delete a node in full, or not
        debug_assert!(existing_node.num_occupied() >= 2);
        if existing_node.num_occupied() == 2 {
            let (short_index, (_, mut existing_node)) = self.branch_nodes.remove_persisted(node.index()).expect("must delete existing");
            dbg!(branch_index);
            dbg!(hex::encode(prefix));
            dbg!(hex::encode(short_index.node_prefix));
            existing_node.child_nodes[branch_index] = NodeType::empty();
            let mut surviving_branch = 16;
            let mut offset = 0;
            let mut len = 0;
            for idx in 0..16 {
                if existing_node.child_nodes[idx].is_empty() == false {
                    len = existing_node.child_encoding_lengths[idx] as usize;
                    surviving_branch = idx;
                    break;
                }
                offset += existing_node.child_encoding_lengths[idx] as usize;
            }
            assert!(surviving_branch < 16);
            let surviving_node = existing_node.child_nodes[surviving_branch];
            let survining_node_encoding = &existing_node.branches_encodings_concatenation[offset..][..len];
            dbg!(hex::encode(survining_node_encoding));
            if surviving_node.is_unreferenced_path() {
                if survining_node_encoding.len() == 33 {
                    // it requires a preimage
                    let key_for_preimage = rlp_parse_short_bytes(survining_node_encoding)?;
                    panic!("require preimage for node value {}", hex::encode(key_for_preimage));
                } else {
                    // we can try to parse it as node
                    todo!();
                }
            }
            // we must take remaining element and "attach" it to parent.
            let parent_node = existing_node.parent_node;
            let remaining_prefix = short_index.node_prefix;
            if parent_node.is_branch() {
                // we will write it into the branch as a leaf node
                let new_leaf = interner.convert_branch_value_into_leaf(
                    branch_index,
                    survining_node_encoding,
                    hasher
                )?;
                self.update_branch_node(
                    parent_node,
                    remaining_prefix,
                    branch_index,
                    new_leaf,
                    surviving_node,
                    interner,
                    hasher
                )
            } else if parent_node.is_extension() {
                // we will need to replace extension as leaf itself
                todo!();
            } else {
                return Err(());
            }
        } else {
            // effectively degrades to update
            let new_branch_key = interner.update_branch_node(existing_node, branch_index, &EMPTY_LIST_ENCODING, hasher)?;
            debug_assert!(new_branch_key.len() <= 33);
            existing_node.child_nodes[branch_index] = NodeType::empty();

            let remaining_prefix = &prefix[..(prefix.len() - 1)];

            if remaining_prefix.is_empty() {
                // Done
                assert!(self.root == node);
                return Ok(new_branch_key);
            } else {
                // walk up
                if remaining_prefix.is_empty() {
                    return Err(());
                }
                let parent_node = existing_node.parent_node;
                if parent_node.is_branch() {
                    let branch_index = *remaining_prefix.last().unwrap() as usize;
                    return self.update_branch_node(
                        parent_node,
                        remaining_prefix,
                        branch_index,
                        new_branch_key,
                        node,
                        interner,
                        hasher,
                    );
                } else if parent_node.is_extension() {
                    return self.update_extension_node(
                        parent_node,
                        remaining_prefix,
                        new_branch_key,
                        node,
                        interner,
                        hasher,
                    );
                } else {
                    return Err(());
                }
            }
        }
    }
}
use super::*;
use crate::system_implementation::ethereum_storage_model::mpt::trie::ShortNodeIndex;

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub(crate) fn find_as_branch<'b>(
        &self,
        full_path: &'b [u8],
    ) -> Result<(NodeType, &'b [u8], usize), ()> {
        // it's a little naive, but we can walk over index of leafs
        // to get all elements for which prefix is less than current, and take few top-most
        let short_index = ShortNodeIndex {
            node_prefix: full_path, // we use full path, and any slice that has same prefix, but shorter - is less than it
        };
        dbg!(hex::encode(short_index.node_prefix));
        for (index, pos) in self.branch_nodes.range(..short_index).rev() {
            dbg!(hex::encode(index.node_prefix));
            if full_path.starts_with(index.node_prefix) == false {
                // we diverge too much
                break;
            }
            if full_path.len() - 1 != index.node_prefix.len() {
                return Err(());
            }
            let branch_index = *full_path.last().unwrap() as usize;
            let remaining_prefix = &full_path[..(full_path.len() - 1)];
            let node_type = NodeType::branch(*pos);
            return Ok((node_type, remaining_prefix, branch_index));
        }

        Err(())
    }

    pub(crate) fn update_branch_node<D: MiniDigest>(
        &mut self,
        node: NodeType,
        prefix: &[u8],
        branch_index: usize,
        pre_encoded_branch_value: &[u8],
        child_node: NodeType,
        interner: &'_ mut (impl Interner<'a> + 'a),
        hasher: &mut D,
    ) -> Result<&'a [u8], ()>
    where
        D::HashOutput: AsRef<[u8]>,
    {
        // dbg!(hex::encode(pre_encoded_branch_value));

        let (_, (_, existing_node)) = self.branch_nodes.get_persistent_mut_by_index(node.index());
        // we need to recompute the key
        let new_branch_key = interner.update_branch_node(
            existing_node,
            branch_index,
            pre_encoded_branch_value,
            hasher,
        )?;
        debug_assert!(new_branch_key.len() <= 33);
        existing_node.child_nodes[branch_index] = child_node;

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

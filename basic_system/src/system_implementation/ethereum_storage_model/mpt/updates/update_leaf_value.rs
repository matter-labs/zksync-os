use core::cmp::Ordering;
use super::*;

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub(crate) fn find_as_existing_leaf<'b>(&self, full_path: &'b [u8]) -> Result<NodeType, ()> {
        // it's a little naive, but we can walk over index of leafs
        // to get all elements for which prefix is less than current, and take few top-most
        let short_index = ShortNodeIndex {
            node_prefix: full_path, // we use full path, and any slice that has same prefix, but shorter - is less than it
        };
        for (index, pos) in self.leaf_nodes.range(..short_index).rev() {
            if full_path.starts_with(index.node_prefix) == false {
                // we diverge too much
                break;
            }
            let segment = &full_path[index.node_prefix.len()..];
            let (_, leaf) = self.leaf_nodes.get_persistent_by_index(*pos);
            match leaf.path_segment.cmp(segment) {
                Ordering::Less => break,
                Ordering::Equal => {
                    let node_type = NodeType::leaf(*pos);
                    return Ok(node_type);
                }
                Ordering::Greater => {
                    continue;
                }
            }
        }

        Err(())
    }

    pub(crate) fn update_leaf_node<D: MiniDigest>(
        &mut self,
        node: NodeType,
        full_path: &[u8],
        pre_encoded_leaf_value: &[u8],
        interner: &'_ mut (impl Interner<'a> + 'a),
        hasher: &mut D,
    ) -> Result<&'a [u8], ()>
    where
        D::HashOutput: AsRef<[u8]>,
    {
        let (_, existing_leaf) = self.leaf_nodes.get_persistent_by_index(node.index());
        let Some(remaining_prefix) = full_path.strip_suffix(existing_leaf.path_segment) else {
            return Err(());
        };
        let new_leaf_key =
            interner.update_leaf_value(&existing_leaf, pre_encoded_leaf_value, hasher)?;
        debug_assert!(new_leaf_key.len() <= 33);

        // dbg!(hex::encode(new_leaf_key));

        if remaining_prefix.is_empty() {
            // Done
            assert!(self.root == node);
            return Ok(new_leaf_key);
        } else {
            // walk up
            let parent_node = existing_leaf.parent_node;
            if parent_node.is_branch() {
                let branch_index = *remaining_prefix.last().unwrap() as usize;
                return self.update_branch_node(
                    parent_node,
                    remaining_prefix,
                    branch_index,
                    new_leaf_key,
                    node,
                    interner,
                    hasher,
                );
            } else if parent_node.is_extension() {
                // can not have extension before leaf
                return Err(());
            } else {
                return Err(());
            }
        }

        Err(())
    }
}

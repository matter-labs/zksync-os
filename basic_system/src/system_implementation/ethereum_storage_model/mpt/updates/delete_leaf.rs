use super::*;

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub(crate) fn delete_leaf_node<D: MiniDigest>(
        &mut self,
        node: NodeType,
        full_path: &[u8],
        interner: &'_ mut (impl Interner<'a> + 'a),
        hasher: &mut D,
    ) -> Result<&'a [u8], ()> where D::HashOutput: AsRef<[u8]> {
        let (short_index, existing_leaf) = self.leaf_nodes.remove_persisted(node.index()).expect("must be existing");
        dbg!(hex::encode(short_index.node_prefix));
        dbg!(hex::encode(existing_leaf.path_segment));
        let Some(remaining_prefix) = full_path.strip_suffix(existing_leaf.path_segment) else {
            return Err(())
        };

        if remaining_prefix.is_empty() {
            // Done
            assert!(self.root == NodeType::empty());
            // emply slice encodes empty state
            return Ok(&[]);
        } else {
            // walk up
            if remaining_prefix.is_empty() {
                return Err(());
            }
            let parent_node = existing_leaf.parent_node;
            if parent_node.is_branch() {
                let branch_index = *remaining_prefix.last().unwrap() as usize;
                return self.delete_from_branch_node(
                    parent_node,
                    remaining_prefix,
                    branch_index,
                    interner,
                    hasher,
                );
            } else if parent_node.is_extension() {
                return Err(());
            } else {
                return Err(());
            }
        }

        Err(())
    }
}
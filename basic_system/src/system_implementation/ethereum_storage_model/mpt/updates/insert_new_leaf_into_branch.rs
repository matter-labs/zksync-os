use super::*;

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub(crate) fn insert_new_leaf_into_existing_branch(
        &mut self,
        branch_node: NodeType,
        branch_index: usize,
        partial_path: Path<'_>,
        pre_encoded_value: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        self.keys_cache.remove(&branch_node);

        let path_segment = interner.intern_slice(partial_path.remaining_path())?;
        let mut value = interner.intern_slice(pre_encoded_value)?;
        let value = RLPSlice::parse(&mut value)?;
        let leaf_node = LeafNode {
            path_segment,
            parent_node: branch_node,
            raw_nibbles_encoding: &[], // it's a fresh one, so we do not benefit from it
            value,
        };
        let node = self.push_leaf(leaf_node);

        let parent_branch = &mut self.branch_nodes[branch_node.index()];
        debug_assert!(parent_branch.child_nodes[branch_index].is_empty());
        parent_branch.child_nodes[branch_index] = node;

        Ok(())
    }
}

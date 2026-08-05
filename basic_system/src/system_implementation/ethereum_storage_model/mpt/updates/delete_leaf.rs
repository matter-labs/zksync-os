use super::*;

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub(crate) fn delete_leaf_node(
        &mut self,
        node: NodeType,
        mut path: Path<'_>,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(), ()> {
        // path is no longer known
        self.keys_cache.remove(&node);

        path.seek_to_end();
        let existing_leaf = &self.leaf_nodes[node.index()];
        path.ascend(&existing_leaf.path_segment);
        let remaining_prefix = path.prefix();

        if remaining_prefix.is_empty() {
            assert_eq!(node, self.root);
            assert!(existing_leaf.parent_node.is_empty());
            self.root = NodeType::empty();
            self.interned_root_node_key = EMPTY_SLICE_ENCODING;

            // Done
            Ok(())
        } else {
            let parent_node = existing_leaf.parent_node;
            debug_assert!(parent_node.is_empty() == false);
            if parent_node.is_branch() {
                self.delete_from_branch_node(parent_node, path, preimages_oracle, interner, hasher)
            } else {
                Err(())
            }
        }
    }
}

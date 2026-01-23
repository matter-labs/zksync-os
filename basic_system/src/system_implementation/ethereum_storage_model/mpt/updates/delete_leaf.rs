use super::*;

impl<'a, A: Allocator + Clone, VC: VecLikeCtor, const COMPARE_HASHES: bool>
    EthereumMPT<'a, A, VC, COMPARE_HASHES>
{
    pub(crate) fn delete_leaf_node(
        &mut self,
        node: NodeType,
        mut path: Path<'_>,
    ) -> Result<(), ()> {
        path.seek_to_end();
        let existing_leaf = &self.capacities.leaf_nodes[node.index()];
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
                let branch_index = path.ascend_branch()?;
                self.delete_from_branch_node(parent_node, branch_index)
            } else {
                Err(())
            }
        }
    }
}

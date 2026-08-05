use super::*;

enum SubtreeDeletionControlFlow {
    Continue { node: NodeType },
    Break,
}

impl<'a, A: Allocator + Clone, VC: VecLikeCtor, const COMPARE_HASHES: bool>
    EthereumMPT<'a, A, VC, COMPARE_HASHES>
{
    fn cascade_delete_subtree_step(
        &mut self,
        node_to_delete: NodeType,
    ) -> Result<SubtreeDeletionControlFlow, ()> {
        let parent = if node_to_delete.is_empty() {
            // Must not delete root itself
            return Err(());
        } else if node_to_delete.is_extension() {
            let extension = &self.capacities.extension_nodes[node_to_delete.index()];
            extension.parent_node
        } else if node_to_delete.is_branch() {
            let branch_node = &self.capacities.branch_nodes[node_to_delete.index()];
            branch_node.parent_node
        } else {
            return Err(());
        };

        if parent.is_empty() {
            // we deleted all the way to the root
            self.root = NodeType::empty();
            Ok(SubtreeDeletionControlFlow::Break)
        } else if parent.is_extension() {
            Ok(SubtreeDeletionControlFlow::Continue { node: parent })
        } else if parent.is_branch() {
            let branch = &mut self.capacities.branch_nodes[parent.index()];
            let mut found_child_idx = 16;
            for (child_idx, child_node) in branch.child_nodes.iter().enumerate() {
                if *child_node == node_to_delete {
                    found_child_idx = child_idx;
                    break;
                }
            }
            assert!(found_child_idx < 16);
            branch.delete(found_child_idx)?;
            if branch.num_occupied() == 0 {
                Ok(SubtreeDeletionControlFlow::Continue { node: parent })
            } else {
                Ok(SubtreeDeletionControlFlow::Break)
            }
        } else {
            Err(())
        }
    }

    pub(crate) fn cascade_delete_subtree(
        &mut self,
        mut node_to_delete: NodeType,
    ) -> Result<(), ()> {
        loop {
            match self.cascade_delete_subtree_step(node_to_delete)? {
                SubtreeDeletionControlFlow::Continue { node } => {
                    node_to_delete = node;
                }
                SubtreeDeletionControlFlow::Break => {
                    self.remove_from_cache(node_to_delete);
                    return Ok(());
                }
            }
        }
    }
}

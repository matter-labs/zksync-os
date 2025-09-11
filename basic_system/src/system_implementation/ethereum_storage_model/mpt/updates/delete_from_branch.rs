use super::*;

impl<'a, A: Allocator + Clone, VC: VecLikeCtor> EthereumMPT<'a, A, VC> {
    pub(crate) fn delete_from_branch_node(
        &mut self,
        branch_node: NodeType,
        branch_index: usize,
    ) -> Result<(), ()> {
        let existing_branch = &mut self.capacities.branch_nodes[branch_node.index()];
        existing_branch.delete(branch_index)?;
        existing_branch.invalidate_cache();

        if existing_branch.num_occupied() == 0 {
            // we will handle it right away - it is easier to recreate later,
            // as now it makes minimal work

            // NOTE: this may delete well beyond the branch itself
            self.cascade_delete_subtree(branch_node)?;

            Ok(())
        } else {
            // NOTE: there is an edge case like:
            // - delete a child from branch, and so it has not enough occupied entries
            // - then insert a leaf under different path, and it would end up in the same branch node

            // So we delay consolidation of such changes for now

            Ok(())
        }
    }
}

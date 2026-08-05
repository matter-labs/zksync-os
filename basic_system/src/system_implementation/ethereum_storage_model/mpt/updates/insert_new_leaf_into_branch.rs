use super::*;

impl<'a, A: Allocator + Clone, VC: VecLikeCtor, const COMPARE_HASHES: bool>
    EthereumMPT<'a, A, VC, COMPARE_HASHES>
{
    pub(crate) fn insert_new_leaf_into_existing_branch(
        &mut self,
        branch_node: NodeType,
        branch_index: usize,
        partial_path: Path<'_>,
        value: LeafValue<'a>,
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        self.remove_from_cache(branch_node);

        let path_segment = interner.intern_slice(partial_path.remaining_path())?;
        let leaf_node = LeafNode {
            cached_key: &[],
            path_segment,
            parent_node: branch_node,
            value,
        };
        let node = self.push_leaf(leaf_node);

        let parent_branch = &mut self.capacities.branch_nodes[branch_node.index()];
        parent_branch.attach(node, branch_index)?;

        Ok(())
    }
}

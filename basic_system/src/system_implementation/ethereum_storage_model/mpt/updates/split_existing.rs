use super::*;

impl<'a, A: Allocator + Clone, VC: VecLikeCtor, const COMPARE_HASHES: bool>
    EthereumMPT<'a, A, VC, COMPARE_HASHES>
{
    // NOTE: all splits are "temporary" as they break the invariant that branch node
    // should have at least two children

    pub(crate) fn temporary_split_existing(
        &mut self,
        grand_parent: NodeType,
        grand_parent_branch_index: usize,
        alternative_node: NodeType,
        common_prefix: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        if alternative_node.is_leaf() {
            self.temporary_split_existing_leaf(
                grand_parent,
                grand_parent_branch_index,
                alternative_node,
                common_prefix,
                interner,
            )
        } else if alternative_node.is_extension() {
            self.temporary_split_existing_extension(
                grand_parent,
                grand_parent_branch_index,
                alternative_node,
                common_prefix,
                interner,
            )
        } else {
            Err(())
        }
    }
}

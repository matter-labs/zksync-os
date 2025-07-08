use super::*;

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub(crate) fn update_leaf_node(
        &mut self,
        node: NodeType,
        pre_encoded_leaf_value: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<&'a [u8], ()> {
        // this node no longer has know key
        self.keys_cache.remove(&node);

        // we only re-allocate a node, and will cascade updates later on
        let existing_leaf = &mut self.leaf_nodes[node.index()];
        // we only need to update the value
        let mut new_leaf_value = interner.intern_slice(pre_encoded_leaf_value)?;
        // we do not detach, and do NOT yet mark parent as dirty
        existing_leaf.value = RLPSlice::parse(&mut new_leaf_value)?;

        Ok(new_leaf_value)
    }
}

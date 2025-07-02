use super::*;

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub(crate) fn update_extension_node<D: MiniDigest>(
        &mut self,
        node: NodeType,
        prefix: &[u8],
        pre_encoded_extension_value: &[u8],
        child_node: NodeType,
        interner: &'_ mut (impl Interner<'a> + 'a),
        hasher: &mut D,
    ) -> Result<&'a [u8], ()> where D::HashOutput: AsRef<[u8]> {
        dbg!(hex::encode(pre_encoded_extension_value));

        let (_, (_, existing_node)) = self.extension_nodes.get_persistent_mut_by_index(node.index());
        let Some(remaining_prefix) = prefix.strip_suffix(existing_node.path_segment) else {
            return Err(());
        };
        existing_node.child_node = child_node;
        // we need to recompute the key
        let new_extension_key = interner.update_extension_value(existing_node, pre_encoded_extension_value, hasher)?;
        debug_assert!(new_extension_key.len() <= 33);

        if remaining_prefix.is_empty() {
            // Done
            assert!(self.root == node);
            dbg!(hex::encode(new_extension_key));
            return Ok(new_extension_key);
        } else {
            // walk up
            if remaining_prefix.is_empty() {
                return Err(());
            }
            let parent_node = existing_node.parent_node;
            if parent_node.is_branch() {
                let branch_index = *remaining_prefix.last().unwrap() as usize;
                return self.update_branch_node(
                    parent_node,
                    remaining_prefix,
                    branch_index,
                    new_extension_key,
                    node,
                    interner,
                    hasher,
                );
            } else if parent_node.is_extension() {
                // can not be two extensions one by one
                return Err(());
            } else {
                return Err(());
            }
        }
    }
}
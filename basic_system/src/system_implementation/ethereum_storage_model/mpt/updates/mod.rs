use super::*;

mod update_leaf_value;
mod update_branch_value;
mod update_extension_value;
mod delete_leaf;
mod delete_from_branch;

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub(crate) fn update<D: MiniDigest>(
        &mut self,
        full_path: &[u8],
        pre_encoded_value: &[u8],
        interner: &'_ mut (impl Interner<'a> + 'a),
        hasher: &mut D,
    ) -> Result<&'a [u8], ()> where D::HashOutput: AsRef<[u8]> {
        // it's either leaf or branch
        if let Ok(leaf_node) = self.find_as_existing_leaf(full_path) {
            self.update_leaf_node(leaf_node, full_path, pre_encoded_value, interner, hasher)
        } else if let Ok((branch_node, prefix, branch_index)) = self.find_as_branch(full_path) {
            self.update_branch_node(branch_node, prefix, branch_index, pre_encoded_value, NodeType::empty(), interner, hasher)
        } else {
            Err(())
        }
    }

    pub(crate) fn delete<D: MiniDigest>(
        &mut self,
        full_path: &[u8],
        interner: &'_ mut (impl Interner<'a> + 'a),
        hasher: &mut D,
    ) -> Result<&'a [u8], ()> where D::HashOutput: AsRef<[u8]> {
        // it's either leaf or branch
        if let Ok(leaf_node) = self.find_as_existing_leaf(full_path) {
            self.delete_leaf_node(leaf_node, full_path, interner, hasher)
        } else if let Ok((branch_node, prefix, branch_index)) = self.find_as_branch(full_path) {
            self.delete_from_branch_node(branch_node, prefix, branch_index, interner, hasher)
        } else {
            Err(())
        }
    }
}
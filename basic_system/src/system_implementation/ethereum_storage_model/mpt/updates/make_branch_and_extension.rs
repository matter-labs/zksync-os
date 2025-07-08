use super::*;

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub(crate) fn make_branch_and_extension(
        &mut self,
        parent_branch_or_empty: NodeType,
        branch_index: usize,
        mut alternative_node: NodeType,
        extension_len: usize,
        partial_path: Path<'_>,
        pre_encoded_value: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        self.keys_cache.remove(&parent_branch_or_empty);

        // very incomplete yet
        let new_branch = BranchNode {
            parent_node: NodeType::empty(),
            child_nodes: [NodeType::empty(); 16],
            _marker: core::marker::PhantomData,
        };
        let new_branch_node = self.push_branch(new_branch);

        // quickly get everything we need from the alternative
        let inscribe_into_branch_directly =
            partial_path.remaining_path().len() == extension_len + 1;
        let alternative_branch_index = if alternative_node.is_leaf() {
            // take existing one and truncate
            let existing_alternative_leaf = &mut self.leaf_nodes[alternative_node.index()];
            existing_alternative_leaf.parent_node = new_branch_node;
            debug_assert_eq!(
                partial_path.remaining_path().len(),
                existing_alternative_leaf.path_segment.len()
            );
            existing_alternative_leaf.path_segment =
                &existing_alternative_leaf.path_segment[extension_len..];
            let alternative_branch_index = existing_alternative_leaf.path_segment[0] as usize;
            existing_alternative_leaf.path_segment = &existing_alternative_leaf.path_segment[1..];
            self.keys_cache.remove(&alternative_node);

            if existing_alternative_leaf.path_segment.is_empty() {
                // it'll go into newly created branch as opaque
                let new_terminal_value = OpaqueValue {
                    parent_node: new_branch_node,
                    branch_index: alternative_branch_index,
                    encoding: existing_alternative_leaf.value,
                };
                let new_terminal_node =
                    NodeType::terminal_value_in_branch(self.branch_terminal_values.len());
                self.branch_terminal_values.push(new_terminal_value);
                alternative_node = new_terminal_node;
            }

            alternative_branch_index
        } else if alternative_node.is_extension() {
            // take existing one and truncate
            let existing_alternative_extension =
                &mut self.extension_nodes[alternative_node.index()];
            existing_alternative_extension.parent_node = new_branch_node;
            debug_assert_eq!(
                partial_path.remaining_path().len(),
                existing_alternative_extension.path_segment.len()
            );
            existing_alternative_extension.path_segment =
                &existing_alternative_extension.path_segment[extension_len..];
            let alternative_branch_index = existing_alternative_extension.path_segment[0] as usize;
            existing_alternative_extension.path_segment =
                &existing_alternative_extension.path_segment[1..];
            self.keys_cache.remove(&alternative_node);

            if existing_alternative_extension.path_segment.is_empty() {
                return Err(());
            }

            alternative_branch_index
        } else {
            return Err(());
        };

        let interned_path = interner.intern_slice(&partial_path.remaining_path())?;
        let (extension_path, rest) = interned_path.split_at(extension_len);

        if inscribe_into_branch_directly == false {
            let (new_leaf_branch_index, path_segment) = rest.split_at(1);
            let new_leaf_branch_index = new_leaf_branch_index[0] as usize;
            debug_assert_ne!(alternative_branch_index, new_leaf_branch_index);

            let mut value = interner.intern_slice(pre_encoded_value)?;
            let value = RLPSlice::parse(&mut value)?;
            let new_leaf = LeafNode {
                path_segment,
                parent_node: new_branch_node,
                raw_nibbles_encoding: &[], // it's a fresh one, so we do not benefit from it
                value,
            };
            let new_leaf_node = self.push_leaf(new_leaf);

            let new_branch_to_update = &mut self.branch_nodes[new_branch_node.index()];
            new_branch_to_update.child_nodes[alternative_branch_index] = alternative_node;
            new_branch_to_update.child_nodes[new_leaf_branch_index] = new_leaf_node;
        } else {
            // we do not make branch indexes
            debug_assert_eq!(rest.len(), 1);
            let new_insert_branch_index = rest[0] as usize;
            debug_assert_ne!(alternative_branch_index, new_insert_branch_index);

            let mut value = interner.intern_slice(pre_encoded_value)?;
            let value = RLPSlice::parse(&mut value)?;

            let new_terminal_value = OpaqueValue {
                parent_node: new_branch_node,
                branch_index: new_insert_branch_index,
                encoding: value,
            };
            let new_terminal_node =
                NodeType::terminal_value_in_branch(self.branch_terminal_values.len());
            self.branch_terminal_values.push(new_terminal_value);

            let new_branch_to_update = &mut self.branch_nodes[new_branch_node.index()];
            new_branch_to_update.child_nodes[alternative_branch_index] = alternative_node;
            new_branch_to_update.child_nodes[new_insert_branch_index] = new_terminal_node;
        }

        if extension_len == 0 {
            self.branch_nodes[new_branch_node.index()].parent_node = parent_branch_or_empty;
            if parent_branch_or_empty.is_branch() {
                // link
                let grand_parent_branch = &mut self.branch_nodes[parent_branch_or_empty.index()];
                grand_parent_branch.child_nodes[branch_index] = new_branch_node;
            } else {
                // mark new root
                self.root = new_branch_node;
            }
        } else {
            // make an extension
            let new_extension = ExtensionNode {
                path_segment: extension_path,
                parent_node: parent_branch_or_empty,
                child_node: new_branch_node,
                raw_nibbles_encoding: &[], // it's a fresh one, so we do not benefit from it
                next_node_key: RLPSlice::empty(),
            };
            let new_extension_node = self.push_extension(new_extension);
            self.branch_nodes[new_branch_node.index()].parent_node = new_extension_node;
            if parent_branch_or_empty.is_branch() {
                // link
                let grand_parent_branch = &mut self.branch_nodes[parent_branch_or_empty.index()];
                grand_parent_branch.child_nodes[branch_index] = new_extension_node;
            } else {
                // mark new root
                self.root = new_extension_node;
            }
        }

        Ok(())
    }
}

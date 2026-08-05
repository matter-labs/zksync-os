use super::*;

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub(crate) fn termorary_split_existing_as_extension_and_branch(
        &mut self,
        grand_parent: NodeType,
        grand_parent_branch_index: usize,
        alternative_node: NodeType,
        extension: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        if alternative_node.is_leaf() {
            self.termorary_split_leaf_into_extension_branch_leaf(
                grand_parent,
                grand_parent_branch_index,
                alternative_node,
                extension,
                interner,
            )
        } else if alternative_node.is_extension() {
            self.termorary_split_existing_extension_as_extension_and_branch(
                grand_parent,
                grand_parent_branch_index,
                alternative_node,
                extension,
                interner,
            )
        } else {
            Err(())
        }
    }

    pub(crate) fn termorary_split_existing_extension_as_extension_and_branch(
        &mut self,
        grand_parent: NodeType,
        grand_parent_branch_index: usize,
        extension_to_split: NodeType,
        extension: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        self.keys_cache.remove(&grand_parent);
        self.keys_cache.remove(&extension_to_split);

        // very incomplete yet
        let new_branch = BranchNode {
            parent_node: NodeType::empty(),
            child_nodes: [NodeType::empty(); 16],
            _marker: core::marker::PhantomData,
        };
        let new_branch_node = self.push_branch(new_branch);

        // take existing one and truncate
        let existing_extension = &mut self.extension_nodes[extension_to_split.index()];
        debug_assert_eq!(existing_extension.parent_node, grand_parent);

        // there is degenerate case when we should replace extension with just another branch node

        let (value_to_place_into_new_branch_node, branch_index) =
            if existing_extension.path_segment.len() == extension.len() + 1 {
                // extension only diverges with another node at the last digit,
                // so it's replaced by another extension below, and it's child need to be placed in newly created branch
                let branch_index = existing_extension.path_segment[extension.len()] as usize;

                if existing_extension.child_node.is_branch() {
                    // update parent of it
                    self.branch_nodes[existing_extension.child_node.index()].parent_node =
                        new_branch_node;
                    (existing_extension.child_node, branch_index)
                } else if existing_extension.child_node.is_unlinked() {
                    // should transform into unreferenced branch value
                    // it'll go into newly created branch as opaque
                    let new_unreferenced_value_value = OpaqueValue {
                        parent_node: new_branch_node,
                        branch_index,
                        encoding: existing_extension.next_node_key,
                    };
                    let new_unreferenced_value_node = NodeType::unreferenced_value_in_branch(
                        self.branch_unreferenced_values.len(),
                    );
                    self.branch_unreferenced_values
                        .push(new_unreferenced_value_value);

                    (new_unreferenced_value_node, branch_index)
                } else {
                    return Err(());
                }
            } else {
                // just need to truncate extension
                existing_extension.parent_node = new_branch_node;
                existing_extension.path_segment =
                    &existing_extension.path_segment[extension.len()..];
                let branch_index = existing_extension.path_segment[0] as usize;
                existing_extension.path_segment = &existing_extension.path_segment[1..];

                debug_assert!(existing_extension.path_segment.is_empty() == false);

                (extension_to_split, branch_index)
            };

        let new_branch_to_update = &mut self.branch_nodes[new_branch_node.index()];
        new_branch_to_update.child_nodes[branch_index] = value_to_place_into_new_branch_node;

        if extension.len() == 0 {
            self.branch_nodes[new_branch_node.index()].parent_node = grand_parent;
            if grand_parent.is_extension() {
                debug_assert_eq!(grand_parent_branch_index, 0);
                let grand_parent_extension = &mut self.extension_nodes[grand_parent.index()];
                grand_parent_extension.child_node = new_branch_node;
            }
            if grand_parent.is_branch() {
                // link
                let grand_parent_branch = &mut self.branch_nodes[grand_parent.index()];
                grand_parent_branch.child_nodes[grand_parent_branch_index] = new_branch_node;
            } else if grand_parent.is_empty() {
                // mark new root
                debug_assert_eq!(grand_parent_branch_index, 0);
                self.root = new_branch_node;
            } else {
                return Err(());
            }
        } else {
            let extension_path = interner.intern_slice(extension)?;
            // make an extension
            let new_extension = ExtensionNode {
                path_segment: extension_path,
                parent_node: grand_parent,
                child_node: new_branch_node,
                raw_nibbles_encoding: &[], // it's a fresh one, so we do not benefit from it
                next_node_key: RLPSlice::empty(),
            };
            let new_extension_node = self.push_extension(new_extension);
            self.branch_nodes[new_branch_node.index()].parent_node = new_extension_node;
            if grand_parent.is_branch() {
                // link
                let grand_parent_branch = &mut self.branch_nodes[grand_parent.index()];
                debug_assert!(
                    grand_parent_branch.child_nodes[grand_parent_branch_index].is_empty() == false
                );
                grand_parent_branch.child_nodes[grand_parent_branch_index] = new_extension_node;
            } else if grand_parent.is_empty() {
                // mark new root
                debug_assert_eq!(grand_parent_branch_index, 0);
                self.root = new_extension_node;
            } else {
                return Err(());
            }
        }

        Ok(())
    }

    pub(crate) fn termorary_split_leaf_into_extension_branch_leaf(
        &mut self,
        grand_parent: NodeType,
        grand_parent_branch_index: usize,
        leaf_to_split: NodeType,
        extension: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        self.keys_cache.remove(&grand_parent);
        self.keys_cache.remove(&leaf_to_split);

        // very incomplete yet
        let new_branch = BranchNode {
            parent_node: NodeType::empty(),
            child_nodes: [NodeType::empty(); 16],
            _marker: core::marker::PhantomData,
        };
        let new_branch_node = self.push_branch(new_branch);

        // take existing one and truncate
        let existing_leaf = &mut self.leaf_nodes[leaf_to_split.index()];
        debug_assert_eq!(existing_leaf.parent_node, grand_parent);

        // there are two case:
        // - either after making extension + branch we do have another (shorter) leaf
        // - or branch is actually just the terminal one

        let (value_to_place_into_new_branch_node, branch_index) =
            if existing_leaf.path_segment.len() == extension.len() + 1 {
                let branch_index = existing_leaf.path_segment[extension.len()] as usize;
                // it'll go into newly created branch as opaque
                let new_terminal_value = OpaqueValue {
                    parent_node: new_branch_node,
                    branch_index,
                    encoding: existing_leaf.value,
                };
                let new_terminal_node =
                    NodeType::terminal_value_in_branch(self.branch_terminal_values.len());
                self.branch_terminal_values.push(new_terminal_value);

                (new_terminal_node, branch_index)
            } else {
                // leaf survives, we just need to shorted it
                existing_leaf.parent_node = new_branch_node;
                existing_leaf.path_segment = &existing_leaf.path_segment[extension.len()..];
                let branch_index = existing_leaf.path_segment[0] as usize;
                existing_leaf.path_segment = &existing_leaf.path_segment[1..];

                (leaf_to_split, branch_index)
            };

        let new_branch_to_update = &mut self.branch_nodes[new_branch_node.index()];
        new_branch_to_update.child_nodes[branch_index] = value_to_place_into_new_branch_node;

        if extension.len() == 0 {
            // attach newly created branch to grand parent
            self.branch_nodes[new_branch_node.index()].parent_node = grand_parent;
            if grand_parent.is_extension() {
                debug_assert_eq!(grand_parent_branch_index, 0);
                let grand_parent_extension = &mut self.extension_nodes[grand_parent.index()];
                grand_parent_extension.child_node = new_branch_node;
            }
            if grand_parent.is_branch() {
                // link
                let grand_parent_branch = &mut self.branch_nodes[grand_parent.index()];
                grand_parent_branch.child_nodes[grand_parent_branch_index] = new_branch_node;
            } else if grand_parent.is_empty() {
                // mark new root
                debug_assert_eq!(grand_parent_branch_index, 0);
                self.root = new_branch_node;
            } else {
                return Err(());
            }
        } else {
            let extension_path = interner.intern_slice(extension)?;
            // make an extension
            let new_extension = ExtensionNode {
                path_segment: extension_path,
                parent_node: grand_parent,
                child_node: new_branch_node,
                raw_nibbles_encoding: &[], // it's a fresh one, so we do not benefit from it
                next_node_key: RLPSlice::empty(),
            };
            let new_extension_node = self.push_extension(new_extension);
            self.branch_nodes[new_branch_node.index()].parent_node = new_extension_node;
            if grand_parent.is_branch() {
                // link
                let grand_parent_branch = &mut self.branch_nodes[grand_parent.index()];
                debug_assert!(
                    grand_parent_branch.child_nodes[grand_parent_branch_index].is_empty() == false
                );
                grand_parent_branch.child_nodes[grand_parent_branch_index] = new_extension_node;
            } else if grand_parent.is_empty() {
                // mark new root
                debug_assert_eq!(grand_parent_branch_index, 0);
                self.root = new_extension_node;
            } else {
                return Err(());
            }
        }

        Ok(())
    }
}

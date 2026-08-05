use super::*;

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub(crate) fn delete_from_branch_node(
        &mut self,
        branch_node: NodeType,
        mut path: Path<'_>,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(), ()> {
        self.keys_cache.remove(&branch_node);

        // here it's a little convoluted, as we may trigger cascading changed
        let branch_index = path.ascend_branch()?;
        let existing_branch = &mut self.branch_nodes[branch_node.index()];
        debug_assert!(existing_branch.num_occupied() >= 2);
        existing_branch.child_nodes[branch_index] = NodeType::empty();

        if existing_branch.num_occupied() >= 2 {
            // easy case - branch node is not deleted by itself

            Ok(())
        } else {
            // cascading case
            let mut surviving_branch = 16;
            for idx in 0..16 {
                if existing_branch.child_nodes[idx].is_empty() == false {
                    surviving_branch = idx;
                    break;
                }
            }
            assert!(surviving_branch < 16);
            let mut surviving_node = existing_branch.child_nodes[surviving_branch];
            let parent = existing_branch.parent_node;
            if surviving_node.is_unreferenced_value_in_branch() {
                // we need to remake a path
                let Path {
                    path: raw_path,
                    prefix_len,
                } = path;
                let new_raw_path = interner.intern_slice_mut(raw_path)?;
                new_raw_path[prefix_len] = surviving_branch as u8;
                let mut modified_path = Path {
                    path: new_raw_path,
                    prefix_len,
                };
                // we have to allocate the next one and decide
                let key = self.branch_unreferenced_values[surviving_node.index()].encoding;
                let current_node = branch_node;
                let parent_branch_index = surviving_branch;
                match self.descend_through_proof(
                    &mut modified_path,
                    key,
                    current_node,
                    preimages_oracle,
                    interner,
                    hasher,
                )? {
                    AppendPath::PathDiverged { allocated_node }
                    | AppendPath::Follow { allocated_node, .. }
                    | AppendPath::LeafReached { allocated_node, .. }
                    | AppendPath::BranchReached {
                        final_branch_node: allocated_node,
                        ..
                    }
                    | AppendPath::EmptyBranchTaken { allocated_node, .. }
                    | AppendPath::BranchTaken { allocated_node, .. } => {
                        debug_assert_ne!(current_node, allocated_node);
                        self.link_if_needed(current_node, parent_branch_index, allocated_node)?;
                        surviving_node = allocated_node;
                    }
                }
            }

            if surviving_node.is_terminal_value_in_branch() {
                todo!();
            } else if surviving_node.is_leaf() {
                // we have to decide based on the parent
                if parent.is_empty() {
                    // remake a root
                    todo!();
                } else if parent.is_extension() {
                    self.attach_leaf_to_higher_extension(
                        surviving_node,
                        parent,
                        branch_node,
                        surviving_branch,
                        path,
                        interner,
                    )
                } else if parent.is_branch() {
                    self.attach_leaf_to_higher_level_branch(
                        surviving_node,
                        parent,
                        branch_node,
                        surviving_branch,
                        path,
                        interner,
                    )
                } else {
                    Err(())
                }
            } else if surviving_node.is_extension() {
                // we will take existing extension, and grow it "up"
                let extension_last_digit = surviving_branch as u8;
                let (grand_parent, extension, grand_parent_branch_index) = if parent.is_extension()
                {
                    // we need to glue them
                    let existing_extension = self.extension_nodes[parent.index()];
                    debug_assert_eq!(existing_extension.child_node, branch_node);
                    path.ascend(existing_extension.path_segment);
                    let grand_parent_branch_index = path.ascend_branch()?;

                    (
                        existing_extension.parent_node,
                        existing_extension.path_segment,
                        grand_parent_branch_index,
                    )
                } else if parent.is_branch() {
                    let grand_parent_branch_index = path.ascend_branch()?;
                    (parent, &[][..], grand_parent_branch_index)
                } else if parent.is_empty() {
                    (parent, &[][..], 0)
                } else {
                    return Err(());
                };
                self.attach_extension_up(
                    surviving_node,
                    grand_parent,
                    grand_parent_branch_index,
                    extension,
                    extension_last_digit,
                    interner,
                )
            } else if surviving_node.is_branch() {
                // we need to replace this branch node with length 1+ extension
                let extension_last_digit = surviving_branch as u8;
                let (grand_parent, extension, grand_parent_branch_index) = if parent.is_extension()
                {
                    // we need to glue them
                    let existing_extension = self.extension_nodes[parent.index()];
                    debug_assert_eq!(existing_extension.child_node, branch_node);
                    path.ascend(existing_extension.path_segment);
                    let grand_parent_branch_index = path.ascend_branch()?;

                    (
                        existing_extension.parent_node,
                        existing_extension.path_segment,
                        grand_parent_branch_index,
                    )
                } else if parent.is_branch() {
                    let grand_parent_branch_index = path.ascend_branch()?;
                    (parent, &[][..], grand_parent_branch_index)
                } else if parent.is_empty() {
                    (parent, &[][..], 0)
                } else {
                    return Err(());
                };

                self.attach_branch_though_extension(
                    surviving_node,
                    grand_parent,
                    grand_parent_branch_index,
                    extension,
                    extension_last_digit,
                    interner,
                )
            } else {
                Err(())
            }
        }
    }

    fn attach_leaf_to_higher_level_branch(
        &mut self,
        existing_leaf_node: NodeType,
        upper_branch_node: NodeType,
        removed_branch_node: NodeType,
        branch_index: usize,
        mut path: Path<'_>,
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        debug_assert_ne!(removed_branch_node, upper_branch_node);
        debug_assert_ne!(removed_branch_node, existing_leaf_node);
        debug_assert_ne!(upper_branch_node, existing_leaf_node);

        self.keys_cache.remove(&existing_leaf_node);
        self.keys_cache.remove(&upper_branch_node);

        let existing_leaf = self.leaf_nodes[existing_leaf_node.index()];
        let mut new_path_buffer = interner.get_buffer(existing_leaf.path_segment.len() + 1)?;
        new_path_buffer.write_byte(branch_index as u8);
        new_path_buffer.write_slice(existing_leaf.path_segment);
        let path_segment = new_path_buffer.flush();
        let leaf_node = LeafNode {
            path_segment,
            parent_node: upper_branch_node,
            raw_nibbles_encoding: &[], // it's a fresh one, so we do not benefit from it
            value: existing_leaf.value,
        };
        let new_leaf_node = self.push_leaf(leaf_node);

        let parent_branch_index = path.ascend_branch()?;
        let parent_branch = &mut self.branch_nodes[upper_branch_node.index()];
        debug_assert_eq!(
            parent_branch.child_nodes[parent_branch_index],
            removed_branch_node
        );
        parent_branch.child_nodes[parent_branch_index] = new_leaf_node;

        Ok(())
    }

    fn attach_leaf_to_higher_extension(
        &mut self,
        existing_leaf_node: NodeType,
        upper_extension_node: NodeType,
        removed_branch_node: NodeType,
        branch_index: usize,
        mut path: Path<'_>,
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        self.keys_cache.remove(&existing_leaf_node);
        self.keys_cache.remove(&upper_extension_node);

        // glue paths together
        let existing_leaf = self.leaf_nodes[existing_leaf_node.index()];
        let parent_extension = self.extension_nodes[upper_extension_node.index()];
        debug_assert_eq!(parent_extension.child_node, removed_branch_node);

        let mut new_path_buffer = interner.get_buffer(
            existing_leaf.path_segment.len() + 1 + parent_extension.path_segment.len(),
        )?;
        new_path_buffer.write_slice(parent_extension.path_segment);
        new_path_buffer.write_byte(branch_index as u8);
        new_path_buffer.write_slice(existing_leaf.path_segment);
        let path_segment = new_path_buffer.flush();

        let grand_parent = parent_extension.parent_node;
        self.keys_cache.remove(&grand_parent);

        path.ascend(parent_extension.path_segment);

        let leaf_node = LeafNode {
            path_segment,
            parent_node: grand_parent,
            raw_nibbles_encoding: &[], // it's a fresh one, so we do not benefit from it
            value: existing_leaf.value,
        };
        let new_leaf_node = self.push_leaf(leaf_node);

        // either root or branch
        if grand_parent.is_empty() {
            self.root = new_leaf_node;

            Ok(())
        } else if grand_parent.is_branch() {
            let grand_parent_branch_index = path.ascend_branch()?;
            let grand_parent_branch = &mut self.branch_nodes[grand_parent.index()];
            debug_assert_eq!(
                grand_parent_branch.child_nodes[grand_parent_branch_index],
                upper_extension_node
            );
            grand_parent_branch.child_nodes[grand_parent_branch_index] = new_leaf_node;

            Ok(())
        } else {
            Err(())
        }
    }

    fn attach_branch_though_extension(
        &mut self,
        existing_branch: NodeType,
        grand_parent: NodeType,
        grand_parent_branch_index: usize,
        extension: &[u8],
        extension_last_digit: u8,
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        debug_assert_ne!(existing_branch, grand_parent);

        self.keys_cache.remove(&existing_branch);
        self.keys_cache.remove(&grand_parent);

        // first create an extension
        let mut buffer = interner.get_buffer(1 + extension.len())?;
        buffer.write_slice(extension);
        buffer.write_byte(extension_last_digit);
        let path_segment = buffer.flush();

        let extension = ExtensionNode {
            parent_node: grand_parent,
            path_segment,
            child_node: existing_branch,
            raw_nibbles_encoding: &[],
            next_node_key: RLPSlice::empty(),
        };
        let extension_node = self.push_extension(extension);
        self.branch_nodes[existing_branch.index()].parent_node = extension_node;

        if grand_parent.is_branch() {
            self.branch_nodes[grand_parent.index()].child_nodes[grand_parent_branch_index] =
                extension_node;

            Ok(())
        } else if grand_parent.is_empty() {
            debug_assert_eq!(grand_parent_branch_index, 0);

            Ok(())
        } else {
            Err(())
        }
    }

    fn attach_extension_up(
        &mut self,
        existing_extension_node: NodeType,
        grand_parent: NodeType,
        grand_parent_branch_index: usize,
        extension: &[u8],
        extension_last_digit: u8,
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        debug_assert_ne!(existing_extension_node, grand_parent);

        self.keys_cache.remove(&existing_extension_node);
        self.keys_cache.remove(&grand_parent);

        // we extend existing extension's path segment "up"

        let existing_extension = &mut self.extension_nodes[existing_extension_node.index()];

        // first create an extension
        let mut buffer =
            interner.get_buffer(1 + extension.len() + existing_extension.path_segment.len())?;
        buffer.write_slice(extension);
        buffer.write_byte(extension_last_digit);
        buffer.write_slice(existing_extension.path_segment);
        let path_segment = buffer.flush();

        existing_extension.parent_node = grand_parent;
        existing_extension.path_segment = path_segment;
        existing_extension.raw_nibbles_encoding = &[];

        if grand_parent.is_branch() {
            self.branch_nodes[grand_parent.index()].child_nodes[grand_parent_branch_index] =
                existing_extension_node;

            Ok(())
        } else if grand_parent.is_empty() {
            debug_assert_eq!(grand_parent_branch_index, 0);

            Ok(())
        } else {
            Err(())
        }
    }
}

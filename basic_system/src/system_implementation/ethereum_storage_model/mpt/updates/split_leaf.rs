use super::*;

impl<'a, A: Allocator + Clone, VC: VecLikeCtor> EthereumMPT<'a, A, VC> {
    // NOTE: all splits are "temporary" as they break the invariant that branch node
    // should have at least two children

    pub(crate) fn temporary_split_existing_leaf(
        &mut self,
        grand_parent: NodeType,
        grand_parent_branch_index: usize,
        leaf_to_split: NodeType,
        new_extension_prefix: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        // take existing one and truncate
        let existing_path_segment_len = self.capacities.leaf_nodes[leaf_to_split.index()]
            .path_segment
            .len();

        if new_extension_prefix.len() == 0 {
            debug_assert!(existing_path_segment_len > 0);
            self.temporary_split_existing_leaf_as_branch_and_leaf(
                grand_parent,
                grand_parent_branch_index,
                leaf_to_split,
            )
        } else {
            debug_assert!(existing_path_segment_len > new_extension_prefix.len());
            self.temporary_split_existing_leaf_as_extension_branch_leaf(
                grand_parent,
                grand_parent_branch_index,
                leaf_to_split,
                new_extension_prefix,
                interner,
            )
        }
    }

    pub(crate) fn temporary_split_existing_leaf_as_branch_and_leaf(
        &mut self,
        grand_parent: NodeType,
        grand_parent_branch_index: usize,
        leaf_to_split: NodeType,
    ) -> Result<(), ()> {
        self.remove_from_cache(grand_parent);
        self.remove_from_cache(leaf_to_split);

        let new_branch = BranchNode {
            cached_key: &[],
            parent_node: grand_parent,
            child_nodes: [NodeType::empty(); 16],
            num_occupied: 0,
            _marker: core::marker::PhantomData,
        };
        let new_branch_node = self.push_branch(new_branch);

        if grand_parent.is_branch() {
            // link
            let grand_parent_branch = &mut self.capacities.branch_nodes[grand_parent.index()];
            debug_assert!(
                grand_parent_branch.child_nodes[grand_parent_branch_index].is_empty() == false
            );
            grand_parent_branch.child_nodes[grand_parent_branch_index] = new_branch_node;
        } else if grand_parent.is_empty() {
            // mark new root
            debug_assert_eq!(grand_parent_branch_index, 0);
            self.root = new_branch_node;
        } else {
            return Err(());
        }

        let existing_leaf = &mut self.capacities.leaf_nodes[leaf_to_split.index()];
        debug_assert_eq!(existing_leaf.parent_node, grand_parent);
        existing_leaf.parent_node = new_branch_node;
        let branch_index_for_existing_leaf = existing_leaf.path_segment[0] as usize;
        existing_leaf.path_segment = &existing_leaf.path_segment[1..];
        #[allow(dropping_references)]
        drop(existing_leaf);

        // and update the branch that we created earlier
        let new_branch_to_update = &mut self.capacities.branch_nodes[new_branch_node.index()];
        new_branch_to_update.attach(leaf_to_split, branch_index_for_existing_leaf)?;

        Ok(())
    }

    pub(crate) fn temporary_split_existing_leaf_as_extension_branch_leaf(
        &mut self,
        grand_parent: NodeType,
        grand_parent_branch_index: usize,
        leaf_to_split: NodeType,
        new_extension_prefix: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        self.remove_from_cache(grand_parent);
        self.remove_from_cache(leaf_to_split);

        let new_branch = BranchNode {
            cached_key: &[],
            parent_node: NodeType::empty(),
            child_nodes: [NodeType::empty(); 16],
            num_occupied: 0,
            _marker: core::marker::PhantomData,
        };
        let new_branch_node = self.push_branch(new_branch);

        let existing_leaf = &mut self.capacities.leaf_nodes[leaf_to_split.index()];
        debug_assert_eq!(existing_leaf.parent_node, grand_parent);
        existing_leaf.parent_node = new_branch_node;
        let branch_index_for_existing_leaf =
            existing_leaf.path_segment[new_extension_prefix.len()] as usize;
        existing_leaf.path_segment =
            &existing_leaf.path_segment[(new_extension_prefix.len() + 1)..];
        #[allow(dropping_references)]
        drop(existing_leaf);

        // now create a new extension and attach it to grand-parent
        let new_prefix_extension = {
            let extension_path = interner.intern_slice(new_extension_prefix)?;
            // make an extension
            let new_extension = ExtensionNode {
                cached_key: &[],
                path_segment: extension_path,
                parent_node: grand_parent,
                child_node: new_branch_node,
            };
            let new_extension_node = self.push_extension(new_extension);
            if grand_parent.is_branch() {
                // link
                let grand_parent_branch = &mut self.capacities.branch_nodes[grand_parent.index()];
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

            new_extension_node
        };

        // and update the branch that we created earlier
        let new_branch_to_update = &mut self.capacities.branch_nodes[new_branch_node.index()];
        new_branch_to_update.parent_node = new_prefix_extension;
        new_branch_to_update.attach(leaf_to_split, branch_index_for_existing_leaf)?;

        Ok(())
    }
}

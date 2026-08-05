use super::*;

impl<'a, A: Allocator + Clone, VC: VecLikeCtor, const COMPARE_HASHES: bool>
    EthereumMPT<'a, A, VC, COMPARE_HASHES>
{
    // NOTE: all splits are "temporary" as they break the invariant that branch node
    // should have at least two children

    pub(crate) fn temporary_split_existing_extension(
        &mut self,
        grand_parent: NodeType,
        grand_parent_branch_index: usize,
        extension_to_split: NodeType,
        new_extension_prefix: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        // take existing one and truncate

        let existing_path_segment_len = self.capacities.extension_nodes[extension_to_split.index()]
            .path_segment
            .len();

        // there is degenerate case when we should replace very short extension with just another branch node
        if existing_path_segment_len == 1 {
            debug_assert_eq!(new_extension_prefix.len(), 0);
            self.temporary_replace_short_extension_with_branch(
                grand_parent,
                grand_parent_branch_index,
                extension_to_split,
            )
        } else if existing_path_segment_len == new_extension_prefix.len() + 1 {
            // extension only diverges with another node at the last digit,
            // so we will create a branch node, but will not need to make one more extension
            // after(!) the branch node
            self.temporary_split_existing_extension_as_extension_and_branch(
                grand_parent,
                extension_to_split,
                new_extension_prefix,
            )
        } else if new_extension_prefix.len() == 0 {
            debug_assert!(existing_path_segment_len > new_extension_prefix.len() + 1);
            // we would need to take extension, replace it's first prefix digit with new branch,
            // and attach to it
            self.temporary_split_existing_extension_as_branch_and_extension(
                grand_parent,
                grand_parent_branch_index,
                extension_to_split,
            )
        } else {
            debug_assert!(existing_path_segment_len > new_extension_prefix.len() + 1);
            // we would need to take extension, remove some of it's prefix,
            // create new extension node and branch, and attach existing extension
            // to the branch
            self.temporary_split_existing_extension_as_extension_branch_extension(
                grand_parent,
                grand_parent_branch_index,
                extension_to_split,
                new_extension_prefix,
                interner,
            )
        }
    }

    pub(crate) fn temporary_replace_short_extension_with_branch(
        &mut self,
        grand_parent: NodeType,
        grand_parent_branch_index: usize,
        extension_to_replace: NodeType,
    ) -> Result<(), ()> {
        self.remove_from_cache(grand_parent);
        self.remove_from_cache(extension_to_replace);

        // very incomplete yet
        let new_branch = BranchNode {
            cached_key: &[],
            parent_node: grand_parent,
            child_nodes: [NodeType::empty(); 16],
            num_occupied: 0,
            _marker: core::marker::PhantomData,
        };
        let new_branch_node = self.push_branch(new_branch);

        // link it instead of extension
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

        let existing_extension = self.capacities.extension_nodes[extension_to_replace.index()];
        debug_assert_eq!(existing_extension.parent_node, grand_parent);
        debug_assert_eq!(existing_extension.path_segment.len(), 1);

        // extension only diverges with another node at the last digit,
        // so it's replaced by another extension below, and it's child need to be placed in newly created branch
        let branch_index = existing_extension.path_segment[0] as usize;
        let child = existing_extension.child_node;

        if child.is_branch() {
            // update parent of it
            let new_branch_to_update = &mut self.capacities.branch_nodes[new_branch_node.index()];
            new_branch_to_update.attach(child, branch_index)?;
            self.capacities.branch_nodes[child.index()].parent_node = new_branch_node;
        } else if child.is_unreferenced_key() {
            let new_branch_to_update = &mut self.capacities.branch_nodes[new_branch_node.index()];
            new_branch_to_update.attach(child, branch_index)?;
            new_branch_to_update.invalidate_cache();
        } else {
            return Err(());
        }

        Ok(())
    }

    pub(crate) fn temporary_split_existing_extension_as_extension_and_branch(
        &mut self,
        grand_parent: NodeType,
        extension_to_split: NodeType,
        new_extension_prefix: &[u8],
    ) -> Result<(), ()> {
        // here we truncate last suffix digit and replace it with a branch node

        let new_branch = BranchNode {
            cached_key: &[],
            parent_node: extension_to_split,
            child_nodes: [NodeType::empty(); 16],
            num_occupied: 0,
            _marker: core::marker::PhantomData,
        };
        let new_branch_node = self.push_branch(new_branch);

        self.remove_from_cache(grand_parent);
        self.remove_from_cache(extension_to_split);

        let existing_extension = &mut self.capacities.extension_nodes[extension_to_split.index()];
        debug_assert_eq!(existing_extension.parent_node, grand_parent);

        debug_assert!(existing_extension.path_segment.len() > new_extension_prefix.len());
        debug_assert_eq!(
            existing_extension.path_segment.len(),
            new_extension_prefix.len() + 1
        );
        debug_assert!(existing_extension.path_segment.len() > 1);

        // we need to truncate last digit of the extension and replace it with branch

        // it is branch index of the child
        let child_to_move = existing_extension.child_node;
        let child_branch_index = existing_extension
            .path_segment
            .last()
            .copied()
            .expect("must be non-empty path") as usize;

        existing_extension.child_node = new_branch_node;
        existing_extension.path_segment =
            &existing_extension.path_segment[..existing_extension.path_segment.len() - 1];
        debug_assert!(existing_extension.path_segment.len() > 0);
        #[allow(dropping_references)]
        drop(existing_extension);

        if child_to_move.is_branch() {
            // update parent of it
            let new_branch_to_update = &mut self.capacities.branch_nodes[new_branch_node.index()];
            new_branch_to_update.attach(child_to_move, child_branch_index)?;
            self.capacities.branch_nodes[child_to_move.index()].parent_node = new_branch_node;
        } else if child_to_move.is_unreferenced_key() {
            let new_branch_to_update = &mut self.capacities.branch_nodes[new_branch_node.index()];
            new_branch_to_update.attach(child_to_move, child_branch_index)?;
            new_branch_to_update.invalidate_cache();
        } else {
            return Err(());
        }

        Ok(())
    }

    pub(crate) fn temporary_split_existing_extension_as_branch_and_extension(
        &mut self,
        grand_parent: NodeType,
        grand_parent_branch_index: usize,
        extension_to_split: NodeType,
    ) -> Result<(), ()> {
        // here we take an existing extension, and trim it's prefix by creating another extension
        // and a branch node

        self.remove_from_cache(grand_parent);
        self.remove_from_cache(extension_to_split);

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

        let existing_extension = &mut self.capacities.extension_nodes[extension_to_split.index()];
        debug_assert_eq!(existing_extension.parent_node, grand_parent);
        existing_extension.parent_node = new_branch_node;
        let branch_index_for_existing_extension = existing_extension.path_segment[0] as usize;
        existing_extension.path_segment = &existing_extension.path_segment[1..];
        debug_assert!(existing_extension.path_segment.len() > 0);
        #[allow(dropping_references)]
        drop(existing_extension);

        // and update the branch that we created earlier
        let new_branch_to_update = &mut self.capacities.branch_nodes[new_branch_node.index()];
        new_branch_to_update.attach(extension_to_split, branch_index_for_existing_extension)?;

        Ok(())
    }

    pub(crate) fn temporary_split_existing_extension_as_extension_branch_extension(
        &mut self,
        grand_parent: NodeType,
        grand_parent_branch_index: usize,
        extension_to_split: NodeType,
        new_extension_prefix: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        // here we take an existing extension, and trim it's prefix by creating another extension
        // and a branch node

        self.remove_from_cache(grand_parent);
        self.remove_from_cache(extension_to_split);

        let new_branch = BranchNode {
            cached_key: &[],
            parent_node: NodeType::empty(),
            child_nodes: [NodeType::empty(); 16],
            num_occupied: 0,
            _marker: core::marker::PhantomData,
        };
        let new_branch_node = self.push_branch(new_branch);

        let existing_extension = &mut self.capacities.extension_nodes[extension_to_split.index()];
        debug_assert_eq!(existing_extension.parent_node, grand_parent);
        existing_extension.parent_node = new_branch_node;
        let branch_index_for_existing_extension =
            existing_extension.path_segment[new_extension_prefix.len()] as usize;
        existing_extension.path_segment =
            &existing_extension.path_segment[(new_extension_prefix.len() + 1)..];
        debug_assert!(existing_extension.path_segment.len() > 0);
        #[allow(dropping_references)]
        drop(existing_extension);

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
        new_branch_to_update.attach(extension_to_split, branch_index_for_existing_extension)?;

        Ok(())
    }
}

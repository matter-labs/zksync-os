use super::*;

enum ReattachControlFlow<'b> {
    ExtendExisting {
        node: NodeType,
        prefix_scratch: &'b mut [u8],
        offset: usize,
    },
    CreateExtension {
        branch: NodeType,
        prefix_scratch: &'b mut [u8],
        offset: usize,
    },
}

impl<'b> ReattachControlFlow<'b> {
    fn add_prefix(&mut self, prefix: &[u8]) -> Result<(), ()> {
        let (dst, offset) = match self {
            Self::ExtendExisting {
                prefix_scratch,
                offset,
                ..
            } => (prefix_scratch, offset),
            Self::CreateExtension {
                prefix_scratch,
                offset,
                ..
            } => (prefix_scratch, offset),
        };
        if *offset < prefix.len() {
            Err(())
        } else {
            dst[(*offset - prefix.len())..*offset].copy_from_slice(prefix);
            *offset -= prefix.len();
            Ok(())
        }
    }
}

impl<'a, A: Allocator + Clone, VC: VecLikeCtor> EthereumMPT<'a, A, VC> {
    pub(crate) fn relink_if_needed(
        &mut self,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(), ()> {
        debug_assert_eq!(self.ensure_linked(), ());

        if self.root.is_empty() {
            Ok(())
        } else if self.root.is_leaf() {
            Ok(())
        } else if self.root.is_extension() {
            if let Some(attachment) =
                self.detach_and_propagate(self.root, preimages_oracle, interner, hasher)?
            {
                self.root = self.collapse_detached(attachment, NodeType::empty(), interner)?;
            }

            Ok(())
        } else if self.root.is_branch() {
            if let Some(attachment) =
                self.detach_and_propagate(self.root, preimages_oracle, interner, hasher)?
            {
                self.root = self.collapse_detached(attachment, NodeType::empty(), interner)?;
            }

            Ok(())
        } else if self.root.is_unreferenced_key() {
            Err(())
        } else if self.root.is_opaque_nontrivial_root() {
            Ok(())
        } else {
            Err(())
        }
    }

    fn detach_and_propagate(
        &mut self,
        node: NodeType,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<Option<ReattachControlFlow<'a>>, ()> {
        if self.get_cached_key(node).is_empty() == false {
            // bail if cached
            return Ok(None);
        }

        if node.is_leaf() {
            Err(())
        } else if node.is_extension() {
            self.detach_extension_node(node, preimages_oracle, interner, hasher)
        } else if node.is_branch() {
            self.detach_branch_node(node, preimages_oracle, interner, hasher)
        } else if node.is_unreferenced_key() {
            Err(())
        } else if node.is_opaque_nontrivial_root() {
            Err(())
        } else {
            Err(())
        }
    }

    fn detach_extension_node(
        &mut self,
        extension_node: NodeType,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<Option<ReattachControlFlow<'a>>, ()> {
        // if we want to drag anything along - we just extend a prefix to drag, otherwise do nothing
        let child = self.capacities.extension_nodes[extension_node.index()].child_node;
        // we should test and descend further - maybe next branch needs to be detached
        if child.is_unlinked() {
            Ok(None)
        } else if child.is_branch() {
            if let Some(mut attachment_form_child) =
                self.detach_and_propagate(child, preimages_oracle, interner, hasher)?
            {
                let extension = &self.capacities.extension_nodes[extension_node.index()];
                attachment_form_child.add_prefix(extension.path_segment)?;
                Ok(Some(attachment_form_child))
            } else {
                Ok(None)
            }
        } else {
            Err(())
        }
    }

    fn make_detached_leaf(
        &mut self,
        leaf_node: NodeType,
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<ReattachControlFlow<'a>, ()> {
        let scratch = interner.intern_slice_mut(&[0u8; 64])?;
        let attachment = ReattachControlFlow::ExtendExisting {
            node: leaf_node,
            prefix_scratch: scratch,
            offset: 64,
        };
        // NOTE: we do not need to add prefix as it's in the leaf itself

        Ok(attachment)
    }

    fn make_detached_extension(
        &self,
        extension_node: NodeType,
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<ReattachControlFlow<'a>, ()> {
        let scratch = interner.intern_slice_mut(&[0u8; 64])?;
        let attachment = ReattachControlFlow::ExtendExisting {
            node: extension_node,
            prefix_scratch: scratch,
            offset: 64,
        };
        // NOTE: we do not need to add prefix as it's in the leaf itself

        Ok(attachment)
    }

    fn make_detached_branch(
        &self,
        branch_node: NodeType,
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<ReattachControlFlow<'a>, ()> {
        let scratch = interner.intern_slice_mut(&[0u8; 64])?;
        let attachment = ReattachControlFlow::CreateExtension {
            branch: branch_node,
            prefix_scratch: scratch,
            offset: 64,
        };

        Ok(attachment)
    }

    fn collapse_detached(
        &mut self,
        attachment: ReattachControlFlow<'a>,
        parent: NodeType,
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<NodeType, ()> {
        self.remove_from_cache(parent);

        match attachment {
            ReattachControlFlow::CreateExtension {
                branch,
                prefix_scratch,
                offset,
            } => {
                // we need to make extension node
                let path = &prefix_scratch[offset..];
                assert!(path.len() > 0);
                let extension = ExtensionNode {
                    cached_key: &[],
                    path_segment: path,
                    parent_node: parent,
                    child_node: branch,
                };
                let extension_node = self.push_extension(extension);
                self.capacities.branch_nodes[branch.index()].parent_node = extension_node;

                Ok(extension_node)
            }
            ReattachControlFlow::ExtendExisting {
                node,
                prefix_scratch,
                offset,
            } => {
                if node.is_extension() {
                    let path = &prefix_scratch[offset..];
                    assert!(path.len() > 0);
                    let detached_extension = &mut self.capacities.extension_nodes[node.index()];
                    let mut buffer =
                        interner.get_buffer(path.len() + detached_extension.path_segment.len())?;
                    debug_assert!(
                        path.len() + detached_extension.path_segment.len() < 64,
                        "total path len is {}",
                        path.len() + detached_extension.path_segment.len()
                    );
                    buffer.write_slice(path);
                    buffer.write_slice(detached_extension.path_segment);
                    let path_segment = buffer.flush();
                    // {
                    //     let path_segment = hex::encode(path_segment).chars().enumerate().filter_map(|(i, el)| {
                    //         if i % 2 == 1 {
                    //             Some(el)
                    //         } else {
                    //             None
                    //         }
                    //     }).collect::<String>();
                    //     dbg!(path_segment);

                    // }

                    detached_extension.parent_node = parent;
                    detached_extension.path_segment = path_segment;

                    self.remove_from_cache(node);

                    Ok(node)
                } else if node.is_leaf() {
                    let path = &prefix_scratch[offset..];
                    assert!(path.len() > 0);
                    let detached_leaf = &mut self.capacities.leaf_nodes[node.index()];
                    let mut buffer =
                        interner.get_buffer(path.len() + detached_leaf.path_segment.len())?;
                    debug_assert!(
                        path.len() + detached_leaf.path_segment.len() <= 64,
                        "total path len is {}",
                        path.len() + detached_leaf.path_segment.len()
                    );
                    buffer.write_slice(path);
                    buffer.write_slice(detached_leaf.path_segment);
                    let path_segment = buffer.flush();
                    // {
                    //     let path_segment = hex::encode(path_segment).chars().enumerate().filter_map(|(i, el)| {
                    //         if i % 2 == 1 {
                    //             Some(el)
                    //         } else {
                    //             None
                    //         }
                    //     }).collect::<String>();
                    //     dbg!(path_segment);

                    // }

                    detached_leaf.parent_node = parent;
                    detached_leaf.path_segment = path_segment;

                    self.remove_from_cache(node);

                    Ok(node)
                } else {
                    Err(())
                }
            }
        }
    }

    fn detach_branch_node(
        &mut self,
        branch_node: NodeType,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<Option<ReattachControlFlow<'a>>, ()> {
        let mut branch = self.capacities.branch_nodes[branch_node.index()];
        assert!(
            branch.num_occupied() > 0,
            "node {:?} must have been deleted",
            branch_node
        );

        let occupied_before = branch.num_occupied();
        for child in branch.child_nodes.iter_mut() {
            if child.is_empty() || child.is_unreferenced_key() || child.is_leaf() {
                continue;
            } else if let Some(attachment_form_child) =
                self.detach_and_propagate(*child, preimages_oracle, interner, hasher)?
            {
                *child = self.collapse_detached(attachment_form_child, branch_node, interner)?;
            }
        }
        debug_assert_eq!(branch.num_occupied(), occupied_before);

        // we reconstructed subtree, and now can detach and remove the branch node itself if needed
        if branch.num_occupied() < 2 {
            for (child_idx, child) in branch.child_nodes.into_iter().enumerate() {
                if child.is_empty() {
                    continue;
                }
                let mut child = child;
                if child.is_unreferenced_key() {
                    let key_encoding = self.capacities.unreferenced_keys[child.index()].cached_key;
                    // path is not important here - just something large enough
                    let mut path = Path {
                        path: &[0u8; 64],
                        prefix_len: 0,
                    };
                    let exposed_node = match self.descend_through_proof(
                        &mut path,
                        key_encoding,
                        branch_node,
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
                            debug_assert_ne!(branch_node, allocated_node);

                            allocated_node
                        }
                    };
                    child = exposed_node;
                }

                if child.is_leaf() {
                    let mut attachment = self.make_detached_leaf(child, interner)?;
                    attachment.add_prefix(&[child_idx as u8])?;
                    return Ok(Some(attachment));
                } else if child.is_extension() {
                    let mut attachment = self.make_detached_extension(child, interner)?;
                    attachment.add_prefix(&[child_idx as u8])?;
                    return Ok(Some(attachment));
                } else if child.is_branch() {
                    let mut attachment = self.make_detached_branch(child, interner)?;
                    attachment.add_prefix(&[child_idx as u8])?;
                    return Ok(Some(attachment));
                } else {
                    return Err(());
                }
            }

            Err(())
        } else {
            // Do not forget to update it
            self.capacities.branch_nodes[branch_node.index()] = branch;

            Ok(None)
        }
    }
}

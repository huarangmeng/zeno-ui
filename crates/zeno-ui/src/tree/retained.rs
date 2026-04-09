//! RetainedComposeTree 持有增量合成所需的全部缓存快照。

use std::collections::{HashMap, HashSet};

use zeno_core::Size;
use zeno_scene::Scene;

use crate::render::FragmentStore;
use crate::{DirtyFlags, DirtyReason, Node, NodeId, layout::LayoutArena};

use super::indexing::DenseNodeStore;

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedComposeTree {
    pub(super) root: Node,
    pub(super) viewport: Size,
    pub(super) layout: LayoutArena,
    pub(super) dense_nodes: DenseNodeStore,
    pub(super) layout_dirty_roots: Vec<NodeId>,
    pub(super) fragments_by_node: FragmentStore,
    pub(super) scene: Scene,
    pub(super) dirty: DirtyFlags,
}

impl RetainedComposeTree {
    #[must_use]
    pub fn new(
        root: Node,
        viewport: Size,
        layout: LayoutArena,
        available_by_node: HashMap<NodeId, Size>,
        fragments_by_node: FragmentStore,
        scene: Scene,
    ) -> Self {
        let dense_nodes = DenseNodeStore::build(&root, &layout, &available_by_node);
        Self {
            root,
            viewport,
            layout,
            dense_nodes,
            layout_dirty_roots: Vec::new(),
            fragments_by_node,
            scene,
            dirty: DirtyFlags::clean(),
        }
    }

    #[must_use]
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    #[must_use]
    pub fn root(&self) -> &Node {
        &self.root
    }

    #[must_use]
    pub const fn dirty(&self) -> DirtyFlags {
        self.dirty
    }

    pub fn replace(
        &mut self,
        root: Node,
        viewport: Size,
        layout: LayoutArena,
        available_by_node: HashMap<NodeId, Size>,
        fragments_by_node: FragmentStore,
        scene: Scene,
    ) {
        let dense_nodes = DenseNodeStore::build(&root, &layout, &available_by_node);
        self.root = root;
        self.viewport = viewport;
        self.layout = layout;
        self.dense_nodes = dense_nodes;
        self.layout_dirty_roots.clear();
        self.fragments_by_node = fragments_by_node;
        self.scene = scene;
        self.dirty = DirtyFlags::clean();
    }

    pub fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark(reason);
        self.dense_nodes.mark_dirty(self.root.id(), reason);
        if reason != DirtyReason::Paint {
            self.layout_dirty_roots.clear();
            self.layout_dirty_roots.push(self.root.id());
        }
    }

    pub fn mark_node_dirty(&mut self, node_id: NodeId, reason: DirtyReason) {
        self.dirty.mark(reason);
        if reason == DirtyReason::Paint {
            self.dense_nodes.mark_dirty(node_id, reason);
            return;
        }

        let mut current = Some(node_id);
        while let Some(id) = current {
            self.dense_nodes.mark_dirty(id, reason);
            current = self.dense_nodes.parent_of(id);
        }
        let candidate = match reason {
            DirtyReason::Layout | DirtyReason::Text => node_id,
            DirtyReason::Order => {
                if self.dense_nodes.is_container_like(node_id) {
                    node_id
                } else {
                    self.layout_root_for(node_id)
                }
            }
            DirtyReason::Structure => self.structure_root_for(node_id),
            DirtyReason::Paint => node_id,
        };
        self.insert_layout_dirty_root(
            candidate,
            matches!(reason, DirtyReason::Order | DirtyReason::Structure),
        );
    }

    #[must_use]
    pub fn dirty_node_ids(&self) -> Vec<NodeId> {
        self.dense_nodes.dirty_node_ids()
    }

    #[must_use]
    pub fn layout_dirty_roots(&self) -> Vec<NodeId> {
        if self.layout_dirty_roots.is_empty() && self.dirty.requires_layout() {
            vec![self.root.id()]
        } else {
            self.layout_dirty_roots.clone()
        }
    }

    #[must_use]
    pub fn layout_for(&self, node_id: NodeId) -> Option<&crate::layout::LayoutSlot> {
        self.dense_nodes.layout_for(node_id)
    }

    #[must_use]
    pub fn dirty_flags_for(&self, node_id: NodeId) -> DirtyFlags {
        self.dense_nodes.dirty_flags_for(node_id)
    }

    #[must_use]
    pub fn available_for(&self, node_id: NodeId) -> Option<Size> {
        self.dense_nodes.available_for(node_id)
    }

    #[must_use]
    pub fn node_ids(&self) -> &[NodeId] {
        self.dense_nodes.node_ids()
    }

    #[must_use]
    pub fn fragments(&self) -> &FragmentStore {
        &self.fragments_by_node
    }

    #[must_use]
    pub fn layout(&self) -> &LayoutArena {
        &self.layout
    }

    pub fn update_fragment(&mut self, node_id: NodeId, fragment: Vec<zeno_scene::DrawCommand>) {
        self.fragments_by_node.insert(node_id, fragment);
        self.dense_nodes.clear_dirty(node_id);
    }

    pub fn apply_layout_state(
        &mut self,
        root: Node,
        viewport: Size,
        layout: LayoutArena,
        available_by_node: HashMap<NodeId, Size>,
    ) {
        let dense_nodes = DenseNodeStore::build(&root, &layout, &available_by_node);
        let valid_ids: HashSet<NodeId> = dense_nodes.node_ids().iter().copied().collect();
        self.fragments_by_node.retain(&valid_ids);
        self.root = root;
        self.viewport = viewport;
        self.layout = layout;
        self.dense_nodes = dense_nodes;
        self.layout_dirty_roots.clear();
        self.dirty = DirtyFlags::clean();
    }

    pub fn replace_scene(&mut self, scene: Scene) {
        self.scene = scene;
        self.dirty = DirtyFlags::clean();
        self.layout_dirty_roots.clear();
        self.dense_nodes.clear_all_dirty();
    }

    pub fn sync_root(&mut self, root: Node) {
        self.root = root;
    }
}

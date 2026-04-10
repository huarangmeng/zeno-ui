//! RetainedComposeTree 持有增量合成所需的全部缓存快照。

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
    pub(super) layout_dirty_roots: Vec<usize>,
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
        available: Vec<Size>,
        fragments_by_node: FragmentStore,
        scene: Scene,
    ) -> Self {
        let dense_nodes = DenseNodeStore::build(layout.index_table().clone(), available);
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
        available: Vec<Size>,
        fragments_by_node: FragmentStore,
        scene: Scene,
    ) {
        let dense_nodes = DenseNodeStore::build(layout.index_table().clone(), available);
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
        let root_index = self
            .dense_nodes
            .index_of(self.root.id())
            .expect("root index should exist");
        self.dense_nodes.mark_dirty_at(root_index, reason);
        if reason != DirtyReason::Paint {
            self.layout_dirty_roots.clear();
            self.layout_dirty_roots.push(root_index);
        }
    }

    pub fn mark_node_dirty(&mut self, node_id: NodeId, reason: DirtyReason) {
        self.dirty.mark(reason);
        let Some(node_index) = self.dense_nodes.index_of(node_id) else {
            return;
        };
        if reason == DirtyReason::Paint {
            self.dense_nodes.mark_dirty_at(node_index, reason);
            return;
        }

        let mut current = Some(node_index);
        while let Some(index) = current {
            self.dense_nodes.mark_dirty_at(index, reason);
            current = self.dense_nodes.parent_index_of(index);
        }
        let candidate_index = match reason {
            DirtyReason::Layout | DirtyReason::Text => node_index,
            DirtyReason::Order => {
                if self.dense_nodes.is_container_like_index(node_index) {
                    node_index
                } else {
                    self.layout_root_index_for(node_index)
                }
            }
            DirtyReason::Structure => self.structure_root_index_for(node_index),
            DirtyReason::Paint => node_index,
        };
        self.insert_layout_dirty_root(
            candidate_index,
            matches!(reason, DirtyReason::Order | DirtyReason::Structure),
        );
    }

    #[must_use]
    pub fn dirty_node_ids(&self) -> Vec<NodeId> {
        self.dense_nodes
            .dirty_indices()
            .into_iter()
            .map(|index| self.dense_nodes.node_id_at(index))
            .collect()
    }

    #[must_use]
    pub fn layout_dirty_roots(&self) -> Vec<NodeId> {
        if self.layout_dirty_roots.is_empty() && self.dirty.requires_layout() {
            vec![self.root.id()]
        } else {
            self.layout_dirty_roots
                .iter()
                .map(|index| self.dense_nodes.node_id_at(*index))
                .collect()
        }
    }

    #[must_use]
    pub fn layout_for(&self, node_id: NodeId) -> Option<&crate::layout::LayoutSlot> {
        self.layout.slot(node_id)
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
        let index = self
            .dense_nodes
            .index_of(node_id)
            .expect("layout index should exist for fragment update");
        self.fragments_by_node.insert_at(index, fragment);
        self.dense_nodes.clear_dirty_at(index);
    }

    pub fn apply_layout_state(
        &mut self,
        root: Node,
        viewport: Size,
        layout: LayoutArena,
        available: Vec<Size>,
    ) {
        let old_index_table = self.layout.index_table().clone();
        let new_index_table = layout.index_table().clone();
        let dense_nodes = DenseNodeStore::build(layout.index_table().clone(), available);
        self.fragments_by_node
            .remap(old_index_table.as_ref(), new_index_table.as_ref());
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

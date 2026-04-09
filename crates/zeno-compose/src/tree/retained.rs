//! RetainedComposeTree 持有增量合成所需的全部缓存快照。

use std::collections::{HashMap, HashSet};

use zeno_core::Size;
use zeno_graphics::{DrawCommand, Scene};

use crate::{DirtyFlags, DirtyReason, Node, NodeId, layout::MeasuredNode};

use super::indexing::{index_container_like_nodes, index_measured_nodes, index_parent_nodes};

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedComposeTree {
    pub(super) root: Node,
    pub(super) viewport: Size,
    pub(super) measured: MeasuredNode,
    pub(super) measured_by_node: HashMap<NodeId, MeasuredNode>,
    pub(super) available_by_node: HashMap<NodeId, Size>,
    pub(super) parent_by_node: HashMap<NodeId, NodeId>,
    pub(super) container_like_nodes: HashSet<NodeId>,
    pub(super) dirty_by_node: HashMap<NodeId, DirtyFlags>,
    pub(super) layout_dirty_roots: HashSet<NodeId>,
    pub(super) fragments_by_node: HashMap<NodeId, Vec<DrawCommand>>,
    pub(super) scene: Scene,
    pub(super) dirty: DirtyFlags,
}

impl RetainedComposeTree {
    #[must_use]
    pub fn new(
        root: Node,
        viewport: Size,
        measured: MeasuredNode,
        available_by_node: HashMap<NodeId, Size>,
        fragments_by_node: HashMap<NodeId, Vec<DrawCommand>>,
        scene: Scene,
    ) -> Self {
        let measured_by_node = index_measured_nodes(&root, &measured);
        let parent_by_node = index_parent_nodes(&root);
        let container_like_nodes = index_container_like_nodes(&root);
        let dirty_by_node = measured_by_node
            .keys()
            .copied()
            .map(|id| (id, DirtyFlags::clean()))
            .collect();
        Self {
            root,
            viewport,
            measured,
            measured_by_node,
            available_by_node,
            parent_by_node,
            container_like_nodes,
            dirty_by_node,
            layout_dirty_roots: HashSet::new(),
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
        measured: MeasuredNode,
        available_by_node: HashMap<NodeId, Size>,
        fragments_by_node: HashMap<NodeId, Vec<DrawCommand>>,
        scene: Scene,
    ) {
        let measured_by_node = index_measured_nodes(&root, &measured);
        let parent_by_node = index_parent_nodes(&root);
        let container_like_nodes = index_container_like_nodes(&root);
        let dirty_by_node = measured_by_node
            .keys()
            .copied()
            .map(|id| (id, DirtyFlags::clean()))
            .collect();
        self.root = root;
        self.viewport = viewport;
        self.measured = measured;
        self.measured_by_node = measured_by_node;
        self.available_by_node = available_by_node;
        self.parent_by_node = parent_by_node;
        self.container_like_nodes = container_like_nodes;
        self.dirty_by_node = dirty_by_node;
        self.layout_dirty_roots.clear();
        self.fragments_by_node = fragments_by_node;
        self.scene = scene;
        self.dirty = DirtyFlags::clean();
    }

    pub fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark(reason);
        if let Some(root_flags) = self.dirty_by_node.get_mut(&self.root.id()) {
            root_flags.mark(reason);
        }
        if reason != DirtyReason::Paint {
            self.layout_dirty_roots.clear();
            self.layout_dirty_roots.insert(self.root.id());
        }
    }

    pub fn mark_node_dirty(&mut self, node_id: NodeId, reason: DirtyReason) {
        self.dirty.mark(reason);
        if reason == DirtyReason::Paint {
            if let Some(node_flags) = self.dirty_by_node.get_mut(&node_id) {
                node_flags.mark(reason);
            }
            return;
        }

        let mut current = Some(node_id);
        while let Some(id) = current {
            if let Some(node_flags) = self.dirty_by_node.get_mut(&id) {
                node_flags.mark(reason);
            }
            current = self.parent_by_node.get(&id).copied();
        }
        let candidate = match reason {
            DirtyReason::Layout | DirtyReason::Text | DirtyReason::Order => node_id,
            DirtyReason::Structure => self.structure_root_for(node_id),
            DirtyReason::Paint => node_id,
        };
        self.insert_layout_dirty_root(candidate);
    }

    #[must_use]
    pub fn dirty_node_ids(&self) -> Vec<NodeId> {
        self.dirty_by_node
            .iter()
            .filter_map(|(node_id, flags)| (!flags.is_clean()).then_some(*node_id))
            .collect()
    }

    #[must_use]
    pub fn layout_dirty_roots(&self) -> Vec<NodeId> {
        if self.layout_dirty_roots.is_empty() && self.dirty.requires_layout() {
            vec![self.root.id()]
        } else {
            self.layout_dirty_roots.iter().copied().collect()
        }
    }

    #[must_use]
    pub fn measured_for(&self, node_id: NodeId) -> Option<&MeasuredNode> {
        self.measured_by_node.get(&node_id)
    }

    #[must_use]
    pub fn dirty_flags_for(&self, node_id: NodeId) -> DirtyFlags {
        self.dirty_by_node
            .get(&node_id)
            .copied()
            .unwrap_or_else(DirtyFlags::clean)
    }

    #[must_use]
    pub fn available_for(&self, node_id: NodeId) -> Option<Size> {
        self.available_by_node.get(&node_id).copied()
    }

    #[must_use]
    pub fn available_map(&self) -> &HashMap<NodeId, Size> {
        &self.available_by_node
    }

    #[must_use]
    pub fn fragments(&self) -> &HashMap<NodeId, Vec<DrawCommand>> {
        &self.fragments_by_node
    }

    #[must_use]
    pub fn measured(&self) -> &MeasuredNode {
        &self.measured
    }

    pub fn update_fragment(&mut self, node_id: NodeId, fragment: Vec<DrawCommand>) {
        self.fragments_by_node.insert(node_id, fragment);
        if let Some(flags) = self.dirty_by_node.get_mut(&node_id) {
            *flags = DirtyFlags::clean();
        }
    }

    pub fn apply_layout_state(
        &mut self,
        root: Node,
        viewport: Size,
        measured: MeasuredNode,
        available_by_node: HashMap<NodeId, Size>,
    ) {
        let measured_by_node = index_measured_nodes(&root, &measured);
        let parent_by_node = index_parent_nodes(&root);
        let container_like_nodes = index_container_like_nodes(&root);
        let valid_ids: HashSet<NodeId> = measured_by_node.keys().copied().collect();
        self.fragments_by_node
            .retain(|node_id, _| valid_ids.contains(node_id));
        self.root = root;
        self.viewport = viewport;
        self.measured = measured;
        self.measured_by_node = measured_by_node;
        self.available_by_node = available_by_node;
        self.parent_by_node = parent_by_node;
        self.container_like_nodes = container_like_nodes;
        self.dirty_by_node = valid_ids
            .into_iter()
            .map(|id| (id, DirtyFlags::clean()))
            .collect();
        self.layout_dirty_roots.clear();
        self.dirty = DirtyFlags::clean();
    }

    pub fn replace_scene(&mut self, scene: Scene) {
        self.scene = scene;
        self.dirty = DirtyFlags::clean();
        self.layout_dirty_roots.clear();
        for flags in self.dirty_by_node.values_mut() {
            *flags = DirtyFlags::clean();
        }
    }

    pub fn sync_root(&mut self, root: Node) {
        self.parent_by_node = index_parent_nodes(&root);
        self.container_like_nodes = index_container_like_nodes(&root);
        self.root = root;
    }
}

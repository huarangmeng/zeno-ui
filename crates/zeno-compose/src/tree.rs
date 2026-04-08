use std::collections::HashMap;

use zeno_core::Size;
use zeno_graphics::{DrawCommand, Scene, SceneBlock};

use crate::{
    layout::{MeasuredKind, MeasuredNode},
    DirtyFlags, DirtyReason, Node, NodeId, NodeKind,
};

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedComposeTree {
    root: Node,
    viewport: Size,
    measured: MeasuredNode,
    measured_by_node: HashMap<NodeId, MeasuredNode>,
    available_by_node: HashMap<NodeId, Size>,
    parent_by_node: HashMap<NodeId, NodeId>,
    dirty_by_node: HashMap<NodeId, DirtyFlags>,
    fragments_by_node: HashMap<NodeId, Vec<DrawCommand>>,
    scene: Scene,
    dirty: DirtyFlags,
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
            dirty_by_node,
            fragments_by_node,
            scene,
            dirty: DirtyFlags::clean(),
        }
    }

    #[must_use]
    pub fn can_reuse(&self, root: &Node, viewport: Size) -> bool {
        self.dirty.is_clean() && self.viewport == viewport && self.root == *root
    }

    #[must_use]
    pub fn can_repaint(&self, root: &Node, viewport: Size) -> bool {
        self.viewport == viewport && self.root == *root
    }

    #[must_use]
    pub fn scene(&self) -> &Scene {
        &self.scene
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
        self.dirty_by_node = dirty_by_node;
        self.fragments_by_node = fragments_by_node;
        self.scene = scene;
        self.dirty = DirtyFlags::clean();
    }

    pub fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark(reason);
        if let Some(root_flags) = self.dirty_by_node.get_mut(&self.root.id()) {
            root_flags.mark(reason);
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
    }

    #[must_use]
    pub fn dirty_node_ids(&self) -> Vec<NodeId> {
        self.dirty_by_node
            .iter()
            .filter_map(|(node_id, flags)| (!flags.is_clean()).then_some(*node_id))
            .collect()
    }

    #[must_use]
    pub fn measured_for(&self, node_id: NodeId) -> Option<&MeasuredNode> {
        self.measured_by_node.get(&node_id)
    }

    #[must_use]
    pub fn available_for(&self, node_id: NodeId) -> Option<Size> {
        self.available_by_node.get(&node_id).copied()
    }

    #[must_use]
    pub fn dirty_for(&self, node_id: NodeId) -> DirtyFlags {
        self.dirty_by_node
            .get(&node_id)
            .copied()
            .unwrap_or_else(DirtyFlags::clean)
    }

    pub fn update_fragment(&mut self, node_id: NodeId, fragment: Vec<DrawCommand>) {
        self.fragments_by_node.insert(node_id, fragment);
        if let Some(flags) = self.dirty_by_node.get_mut(&node_id) {
            *flags = DirtyFlags::clean();
        }
    }

    pub fn rebuild_scene_from_fragments(&mut self) {
        let mut blocks = Vec::new();
        collect_blocks_in_order(
            &self.root,
            &self.measured_by_node,
            &self.fragments_by_node,
            &mut blocks,
        );
        self.scene = Scene::from_blocks(self.viewport, blocks);
        self.dirty = DirtyFlags::clean();
    }
}

fn index_measured_nodes(root: &Node, measured: &MeasuredNode) -> HashMap<NodeId, MeasuredNode> {
    let mut indexed = HashMap::new();
    collect_measured_nodes(root, measured, &mut indexed);
    indexed
}

fn index_parent_nodes(root: &Node) -> HashMap<NodeId, NodeId> {
    let mut indexed = HashMap::new();
    collect_parent_nodes(root, &mut indexed);
    indexed
}

fn collect_measured_nodes(
    node: &Node,
    measured: &MeasuredNode,
    indexed: &mut HashMap<NodeId, MeasuredNode>,
) {
    indexed.insert(node.id(), measured.clone());

    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            collect_measured_nodes(child, measured_child, indexed);
        }
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            for (child, measured_child) in children.iter().zip(measured_children.iter()) {
                collect_measured_nodes(child, measured_child, indexed);
            }
        }
        _ => {}
    }
}

fn collect_parent_nodes(node: &Node, indexed: &mut HashMap<NodeId, NodeId>) {
    match &node.kind {
        NodeKind::Container(child) => {
            indexed.insert(child.id(), node.id());
            collect_parent_nodes(child, indexed);
        }
        NodeKind::Stack { children, .. } => {
            for child in children {
                indexed.insert(child.id(), node.id());
                collect_parent_nodes(child, indexed);
            }
        }
        _ => {}
    }
}

fn collect_blocks_in_order(
    node: &Node,
    measured_by_node: &HashMap<NodeId, MeasuredNode>,
    fragments_by_node: &HashMap<NodeId, Vec<DrawCommand>>,
    blocks: &mut Vec<SceneBlock>,
) {
    if let (Some(fragment), Some(measured)) = (fragments_by_node.get(&node.id()), measured_by_node.get(&node.id())) {
        blocks.push(SceneBlock::new(node.id().0, blocks.len() as u32, measured.frame, fragment.clone()));
    }

    match &node.kind {
        NodeKind::Container(child) => {
            collect_blocks_in_order(child, measured_by_node, fragments_by_node, blocks);
        }
        NodeKind::Stack { children, .. } => {
            for child in children {
                collect_blocks_in_order(child, measured_by_node, fragments_by_node, blocks);
            }
        }
        _ => {}
    }
}

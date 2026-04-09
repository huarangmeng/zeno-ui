use std::collections::{HashMap, HashSet};

use zeno_core::Size;
use zeno_graphics::{DrawCommand, Scene};

use crate::{
    DirtyFlags, DirtyReason, Node, NodeId, NodeKind,
    layout::{MeasuredKind, MeasuredNode},
};

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedComposeTree {
    root: Node,
    viewport: Size,
    measured: MeasuredNode,
    measured_by_node: HashMap<NodeId, MeasuredNode>,
    available_by_node: HashMap<NodeId, Size>,
    parent_by_node: HashMap<NodeId, NodeId>,
    container_like_nodes: HashSet<NodeId>,
    dirty_by_node: HashMap<NodeId, DirtyFlags>,
    layout_dirty_roots: HashSet<NodeId>,
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
            DirtyReason::Layout | DirtyReason::Text => node_id,
            DirtyReason::Order => node_id,
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

    fn layout_root_for(&self, node_id: NodeId) -> NodeId {
        self.parent_by_node
            .get(&node_id)
            .copied()
            .unwrap_or(node_id)
    }

    fn structure_root_for(&self, node_id: NodeId) -> NodeId {
        if self.container_like_nodes.contains(&node_id) {
            node_id
        } else {
            self.layout_root_for(node_id)
        }
    }

    fn insert_layout_dirty_root(&mut self, candidate: NodeId) {
        loop {
            if self
                .layout_dirty_roots
                .iter()
                .any(|existing| self.is_ancestor_or_same(*existing, candidate))
            {
                return;
            }
            let to_remove: Vec<NodeId> = self
                .layout_dirty_roots
                .iter()
                .copied()
                .filter(|existing| self.is_ancestor_or_same(candidate, *existing))
                .collect();
            for existing in to_remove {
                self.layout_dirty_roots.remove(&existing);
            }
            self.layout_dirty_roots.insert(candidate);
            return;
        }
    }

    fn is_ancestor_or_same(&self, ancestor: NodeId, mut node_id: NodeId) -> bool {
        if ancestor == node_id {
            return true;
        }
        while let Some(parent) = self.parent_by_node.get(&node_id).copied() {
            if parent == ancestor {
                return true;
            }
            node_id = parent;
        }
        false
    }

    pub fn has_descendant_in(
        &self,
        ancestor: NodeId,
        set: &std::collections::HashSet<NodeId>,
    ) -> bool {
        for candidate in set {
            if *candidate != ancestor && self.is_ancestor_or_same(ancestor, *candidate) {
                return true;
            }
        }
        false
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

fn index_container_like_nodes(root: &Node) -> HashSet<NodeId> {
    let mut indexed = HashSet::new();
    collect_container_like_nodes(root, &mut indexed);
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

fn collect_container_like_nodes(node: &Node, indexed: &mut HashSet<NodeId>) {
    match &node.kind {
        NodeKind::Container(child) => {
            indexed.insert(node.id());
            collect_container_like_nodes(child, indexed);
        }
        NodeKind::Stack { children, .. } => {
            indexed.insert(node.id());
            for child in children {
                collect_container_like_nodes(child, indexed);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::RetainedComposeTree;
    use crate::{
        DirtyReason, Node, NodeId, layout::measure_node, text, widgets::column, widgets::row,
    };
    use zeno_core::{Point, Size};
    use zeno_graphics::Scene;
    use zeno_text::FallbackTextSystem;

    #[test]
    fn text_dirty_keeps_leaf_as_layout_root() {
        let root = column(vec![text("Title").key("title"), text("Body").key("body")]).key("root");
        let body_id = text("Body").key("body").id();
        let mut retained = retained_tree_for(root, Size::new(320.0, 240.0));

        retained.mark_node_dirty(body_id, DirtyReason::Text);

        assert_eq!(sorted_roots(&retained), vec![body_id]);
    }

    #[test]
    fn structure_dirty_promotes_to_parent_layout_root() {
        let root = column(vec![text("Title").key("title"), text("Body").key("body")]).key("root");
        let root_id = root.id();
        let body_id = text("Body").key("body").id();
        let mut retained = retained_tree_for(root, Size::new(320.0, 240.0));

        retained.mark_node_dirty(body_id, DirtyReason::Structure);

        assert_eq!(sorted_roots(&retained), vec![root_id]);
    }

    #[test]
    fn structure_dirty_on_container_stays_local_to_container_root() {
        let card = column(vec![text("Title").key("title"), text("Body").key("body")]).key("card");
        let card_id = card.id();
        let root = row(vec![card, text("Side").key("side")]).key("root");
        let mut retained = retained_tree_for(root, Size::new(320.0, 240.0));

        retained.mark_node_dirty(card_id, DirtyReason::Structure);

        assert_eq!(sorted_roots(&retained), vec![card_id]);
    }

    #[test]
    fn order_dirty_keeps_stack_node_as_local_root() {
        let stack = column(vec![text("A").key("a"), text("B").key("b")]).key("stack");
        let stack_id = stack.id();
        let root = row(vec![stack, text("Side").key("side")]).key("root");
        let mut retained = retained_tree_for(root, Size::new(320.0, 240.0));

        retained.mark_node_dirty(stack_id, DirtyReason::Order);

        assert_eq!(sorted_roots(&retained), vec![stack_id]);
    }

    #[test]
    fn sibling_dirty_nodes_remain_independent_leaf_roots() {
        let root = column(vec![text("A").key("a"), text("B").key("b")]).key("root");
        let id_a = text("A").key("a").id();
        let id_b = text("B").key("b").id();
        let mut retained = retained_tree_for(root, Size::new(320.0, 240.0));
        retained.mark_node_dirty(id_a, DirtyReason::Layout);
        retained.mark_node_dirty(id_b, DirtyReason::Layout);
        let roots = sorted_roots(&retained);
        assert_eq!(roots, vec![id_a, id_b]);
    }

    #[test]
    fn dirty_nodes_in_different_containers_stay_scoped_to_their_branches() {
        let left = column(vec![text("L1").key("l1"), text("L2").key("l2")]).key("left");
        let right = column(vec![text("R1").key("r1"), text("R2").key("r2")]).key("right");
        let root = row(vec![left, right]).key("root");
        let id_l2 = text("L2").key("l2").id();
        let id_r2 = text("R2").key("r2").id();
        let mut retained = retained_tree_for(root, Size::new(320.0, 240.0));
        retained.mark_node_dirty(id_l2, DirtyReason::Layout);
        retained.mark_node_dirty(id_r2, DirtyReason::Layout);
        let roots = sorted_roots(&retained);
        assert_eq!(roots, vec![id_l2, id_r2]);
    }

    fn retained_tree_for(root: Node, viewport: Size) -> RetainedComposeTree {
        let measured = measure_node(&root, Point::new(0.0, 0.0), viewport, &FallbackTextSystem);
        RetainedComposeTree::new(
            root,
            viewport,
            measured,
            HashMap::new(),
            HashMap::new(),
            Scene::new(viewport),
        )
    }

    fn sorted_roots(retained: &RetainedComposeTree) -> Vec<NodeId> {
        let mut roots = retained.layout_dirty_roots();
        roots.sort_by_key(|id| id.0);
        roots
    }
}

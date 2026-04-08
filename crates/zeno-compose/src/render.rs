use std::collections::{HashMap, HashSet};

use zeno_core::{Point, Size};
use zeno_graphics::{Brush, DrawCommand, Scene, ScenePatch, SceneSubmit, Shape};
use zeno_text::TextSystem;

use crate::{
    invalidation::DirtyReason,
    layout::{measure_node, MeasuredKind, MeasuredNode},
    tree::RetainedComposeTree,
    Node, NodeId, NodeKind,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ComposeStats {
    pub compose_passes: usize,
    pub layout_passes: usize,
    pub cache_hits: usize,
}

pub struct ComposeRenderer<'a> {
    text_system: &'a dyn TextSystem,
}

impl<'a> ComposeRenderer<'a> {
    #[must_use]
    pub fn new(text_system: &'a dyn TextSystem) -> Self {
        Self { text_system }
    }

    #[must_use]
    pub fn compose(&self, root: &Node, viewport: Size) -> Scene {
        compose_scene_internal(root, viewport, self.text_system)
    }
}

pub struct ComposeEngine<'a> {
    text_system: &'a dyn TextSystem,
    retained: Option<RetainedComposeTree>,
    stats: ComposeStats,
}

impl<'a> ComposeEngine<'a> {
    #[must_use]
    pub fn new(text_system: &'a dyn TextSystem) -> Self {
        Self {
            text_system,
            retained: None,
            stats: ComposeStats::default(),
        }
    }

    #[must_use]
    pub fn compose(&mut self, root: &Node, viewport: Size) -> Scene {
        match self.compose_submit(root, viewport) {
            SceneSubmit::Full(scene) => scene,
            SceneSubmit::Patch { current, .. } => current,
        }
    }

    #[must_use]
    pub fn compose_submit(&mut self, root: &Node, viewport: Size) -> SceneSubmit {
        if let Some(retained) = self.retained.as_mut() {
            if retained.scene().size == viewport && retained.root() != root {
                reconcile_root_change(retained, root);
            }
        }

        if let Some(retained) = self.retained.as_mut() {
            if retained.dirty().is_clean() && retained.scene().size == viewport {
                if retained.root() != root {
                    retained.sync_root(root.clone());
                }
                self.stats.cache_hits += 1;
                return SceneSubmit::Full(retained.scene().clone());
            }
        }

        if let Some(retained) = self.retained.as_mut() {
            if retained.dirty().requires_paint_only() && retained.scene().size == viewport {
                self.stats.compose_passes += 1;
                let previous_scene = retained.scene().clone();
                repaint_dirty_nodes(root, retained);
                retained.sync_root(root.clone());
                let scene = retained.scene().clone();
                let patch = diff_scenes(&previous_scene, &scene);
                return if patch.is_empty() {
                    SceneSubmit::Full(scene)
                } else {
                    SceneSubmit::Patch { patch, current: scene }
                };
            }
        }

        if let Some(retained) = self.retained.as_mut() {
            if retained.dirty().requires_layout() && retained.scene().size == viewport {
                self.stats.compose_passes += 1;
                self.stats.layout_passes += 1;
                let previous_scene = retained.scene().clone();
                let layout_dirty_roots: HashSet<NodeId> =
                    retained.layout_dirty_roots().into_iter().collect();
                let measured = relayout_node(
                    root,
                    Point::new(0.0, 0.0),
                    viewport,
                    self.text_system,
                    retained,
                    &layout_dirty_roots,
                    false,
                )
                .0;
                let (available_by_node, fragments_by_node, scene) =
                    structured_scene_from_measured(root, viewport, &measured);
                retained.replace(
                    root.clone(),
                    viewport,
                    measured,
                    available_by_node,
                    fragments_by_node,
                    scene.clone(),
                );
                let patch = diff_scenes(&previous_scene, &scene);
                return if patch.is_empty() {
                    SceneSubmit::Full(scene)
                } else {
                    SceneSubmit::Patch { patch, current: scene }
                };
            }
        }

        self.stats.compose_passes += 1;
        self.stats.layout_passes += 1;
        let measured = measure_node(root, Point::new(0.0, 0.0), viewport, self.text_system);
        let (available_by_node, fragments_by_node, scene) =
            structured_scene_from_measured(root, viewport, &measured);
        match self.retained.as_mut() {
            Some(retained) => retained.replace(
                root.clone(),
                viewport,
                measured,
                available_by_node,
                fragments_by_node,
                scene.clone(),
            ),
            None => {
                self.retained = Some(RetainedComposeTree::new(
                    root.clone(),
                    viewport,
                    measured,
                    available_by_node,
                    fragments_by_node,
                    scene.clone(),
                ));
            }
        }
        SceneSubmit::Full(scene)
    }

    pub fn invalidate(&mut self, reason: DirtyReason) {
        if let Some(retained) = self.retained.as_mut() {
            retained.mark_dirty(reason);
        }
    }

    pub fn invalidate_node(&mut self, node_id: NodeId, reason: DirtyReason) {
        if let Some(retained) = self.retained.as_mut() {
            retained.mark_node_dirty(node_id, reason);
        }
    }

    #[must_use]
    pub fn current_scene(&self) -> Option<&Scene> {
        self.retained.as_ref().map(RetainedComposeTree::scene)
    }

    #[must_use]
    pub const fn stats(&self) -> ComposeStats {
        self.stats
    }
}

#[must_use]
pub fn compose_scene(root: &Node, viewport: Size, text_system: &dyn TextSystem) -> Scene {
    compose_scene_internal(root, viewport, text_system)
}

fn compose_scene_internal(root: &Node, viewport: Size, text_system: &dyn TextSystem) -> Scene {
    let measured = measure_node(root, Point::new(0.0, 0.0), viewport, text_system);
    structured_scene_from_measured(root, viewport, &measured).2
}

fn structured_scene_from_measured(
    root: &Node,
    viewport: Size,
    measured: &MeasuredNode,
) -> (HashMap<NodeId, Size>, HashMap<NodeId, Vec<DrawCommand>>, Scene) {
    let mut fragments_by_node = HashMap::new();
    let mut available_by_node = HashMap::new();
    collect_fragments(
        root,
        measured,
        viewport,
        &mut available_by_node,
        &mut fragments_by_node,
    );
    let scene = build_scene(root, measured, viewport, &fragments_by_node);
    (available_by_node, fragments_by_node, scene)
}

fn collect_fragments(
    node: &Node,
    measured: &MeasuredNode,
    available: Size,
    available_by_node: &mut HashMap<NodeId, Size>,
    fragments_by_node: &mut HashMap<NodeId, Vec<DrawCommand>>,
) {
    available_by_node.insert(node.id(), available);
    fragments_by_node.insert(node.id(), node_fragment(node, measured));

    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            collect_fragments(
                child,
                measured_child,
                container_child_available(node, available),
                available_by_node,
                fragments_by_node,
            );
        }
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            collect_stack_fragments(
                node,
                children,
                measured_children,
                available,
                available_by_node,
                fragments_by_node,
            );
        }
        _ => {}
    }
}

fn node_fragment(node: &Node, measured: &MeasuredNode) -> Vec<DrawCommand> {
    let mut fragment = Vec::new();
    if let Some(background) = node.style.background {
        let shape = if node.style.corner_radius > 0.0 {
            Shape::RoundedRect {
                rect: measured.frame,
                radius: node.style.corner_radius,
            }
        } else {
            Shape::Rect(measured.frame)
        };
        fragment.push(DrawCommand::Fill {
            shape,
            brush: Brush::Solid(background),
        });
    }

    match (&node.kind, &measured.kind) {
        (NodeKind::Text(_), MeasuredKind::Text(layout)) => {
            let position = Point::new(
                measured.frame.origin.x + node.style.padding.left,
                measured.frame.origin.y + node.style.padding.top + layout.paragraph.font_size,
            );
            fragment.push(DrawCommand::Text {
                position,
                layout: layout.clone(),
                color: node.style.foreground,
            });
        }
        _ => {}
    }

    fragment
}

fn repaint_dirty_nodes(root: &Node, retained: &mut RetainedComposeTree) {
    let dirty_node_ids = retained.dirty_node_ids();
    for node_id in dirty_node_ids {
        if let (Some(node), Some(measured)) = (
            find_node(root, node_id),
            retained.measured_for(node_id).cloned(),
        ) {
            retained.update_fragment(node_id, node_fragment(node, &measured));
        }
    }
    retained.rebuild_scene_from_fragments();
}

fn find_node(node: &Node, node_id: NodeId) -> Option<&Node> {
    if node.id() == node_id {
        return Some(node);
    }

    match &node.kind {
        NodeKind::Container(child) => find_node(child, node_id),
        NodeKind::Stack { children, .. } => children.iter().find_map(|child| find_node(child, node_id)),
        _ => None,
    }
}

fn build_scene(
    root: &Node,
    measured: &MeasuredNode,
    viewport: Size,
    fragments_by_node: &HashMap<NodeId, Vec<DrawCommand>>,
) -> Scene {
    let mut blocks = Vec::new();
    collect_scene_blocks(root, measured, fragments_by_node, &mut blocks);
    Scene::from_blocks(viewport, blocks)
}

fn diff_scenes(previous: &Scene, current: &Scene) -> ScenePatch {
    let previous_by_id: HashMap<u64, &zeno_graphics::SceneBlock> = previous
        .blocks
        .iter()
        .map(|block| (block.node_id, block))
        .collect();
    let upserts = current
        .blocks
        .iter()
        .filter(|block| previous_by_id.get(&block.node_id).copied() != Some(block))
        .cloned()
        .collect();
    let removes = previous
        .blocks
        .iter()
        .filter(|block| !current.blocks.iter().any(|current_block| current_block.node_id == block.node_id))
        .map(|block| block.node_id)
        .collect();
    ScenePatch {
        size: current.size,
        base_block_count: previous.blocks.len(),
        upserts,
        removes,
    }
}

fn reconcile_root_change(retained: &mut RetainedComposeTree, root: &Node) {
    let previous_root = retained.root().clone();
    if previous_root.id() != root.id() {
        retained.mark_dirty(DirtyReason::Structure);
        return;
    }
    let mut previous_by_id = HashMap::new();
    index_nodes(&previous_root, &mut previous_by_id);
    reconcile_node(retained, &previous_by_id, root);
}

fn reconcile_node<'a>(
    retained: &mut RetainedComposeTree,
    previous_by_id: &HashMap<NodeId, &'a Node>,
    current: &Node,
) {
    match previous_by_id.get(&current.id()).copied() {
        Some(previous) => {
            if let Some(reason) = local_change_reason(previous, current) {
                retained.mark_node_dirty(current.id(), reason);
            }
        }
        None => {
            retained.mark_dirty(DirtyReason::Structure);
            return;
        }
    }

    match &current.kind {
        NodeKind::Container(child) => reconcile_node(retained, previous_by_id, child),
        NodeKind::Stack { children, .. } => {
            for child in children {
                reconcile_node(retained, previous_by_id, child);
            }
        }
        _ => {}
    }
}

fn index_nodes<'a>(node: &'a Node, indexed: &mut HashMap<NodeId, &'a Node>) {
    indexed.insert(node.id(), node);
    match &node.kind {
        NodeKind::Container(child) => index_nodes(child, indexed),
        NodeKind::Stack { children, .. } => {
            for child in children {
                index_nodes(child, indexed);
            }
        }
        _ => {}
    }
}

fn local_change_reason(previous: &Node, current: &Node) -> Option<DirtyReason> {
    if previous.id() != current.id() {
        return Some(DirtyReason::Structure);
    }

    match (&previous.kind, &current.kind) {
        (NodeKind::Text(previous_text), NodeKind::Text(current_text)) => {
            if previous_text.content != current_text.content
                || previous_text.font != current_text.font
                || previous_text.font_size != current_text.font_size
            {
                return Some(DirtyReason::Text);
            }
            style_change_reason(previous, current, true, false)
        }
        (NodeKind::Spacer(previous_spacer), NodeKind::Spacer(current_spacer)) => {
            if previous_spacer != current_spacer {
                return Some(DirtyReason::Layout);
            }
            style_change_reason(previous, current, false, false)
        }
        (NodeKind::Container(previous_child), NodeKind::Container(current_child)) => {
            if previous_child.id() != current_child.id() {
                return Some(DirtyReason::Structure);
            }
            style_change_reason(previous, current, false, false)
        }
        (
            NodeKind::Stack {
                axis: previous_axis,
                children: previous_children,
            },
            NodeKind::Stack {
                axis: current_axis,
                children: current_children,
            },
        ) => {
            if previous_axis != current_axis || child_ids(previous_children) != child_ids(current_children) {
                return Some(DirtyReason::Structure);
            }
            style_change_reason(previous, current, false, true)
        }
        _ => Some(DirtyReason::Structure),
    }
}

fn style_change_reason(
    previous: &Node,
    current: &Node,
    text_node: bool,
    stack_node: bool,
) -> Option<DirtyReason> {
    let previous_style = &previous.style;
    let current_style = &current.style;
    if previous_style.padding != current_style.padding
        || previous_style.width != current_style.width
        || previous_style.height != current_style.height
        || (stack_node && previous_style.spacing != current_style.spacing)
    {
        return Some(DirtyReason::Layout);
    }
    if previous_style.background != current_style.background
        || previous_style.corner_radius != current_style.corner_radius
        || (text_node && previous_style.foreground != current_style.foreground)
    {
        return Some(DirtyReason::Paint);
    }
    None
}

fn child_ids(children: &[Node]) -> Vec<NodeId> {
    children.iter().map(Node::id).collect()
}

fn collect_scene_blocks(
    node: &Node,
    measured: &MeasuredNode,
    fragments_by_node: &HashMap<NodeId, Vec<DrawCommand>>,
    blocks: &mut Vec<zeno_graphics::SceneBlock>,
) {
    if let Some(fragment) = fragments_by_node.get(&node.id()) {
        blocks.push(zeno_graphics::SceneBlock::new(
            node.id().0,
            blocks.len() as u32,
            measured.frame,
            fragment.clone(),
        ));
    }

    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            collect_scene_blocks(child, measured_child, fragments_by_node, blocks);
        }
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            for (child, measured_child) in children.iter().zip(measured_children.iter()) {
                collect_scene_blocks(child, measured_child, fragments_by_node, blocks);
            }
        }
        _ => {}
    }
}

fn relayout_node(
    node: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    retained: &RetainedComposeTree,
    layout_dirty_roots: &HashSet<NodeId>,
    force_relayout: bool,
) -> (MeasuredNode, bool) {
    let dirty = layout_dirty_roots.contains(&node.id());
    if !force_relayout && !dirty {
        if let (Some(measured), Some(cached_available)) =
            (retained.measured_for(node.id()), retained.available_for(node.id()))
        {
            if cached_available == available && measured.frame.origin == origin {
                return (measured.clone(), true);
            }
        }
    }

    match &node.kind {
        NodeKind::Text(text) => (
            crate::layout::measure_text(node, text, origin, available, text_system),
            false,
        ),
        NodeKind::Spacer(spacer) => (
            crate::layout::measure_spacer(node, spacer, origin, available),
            false,
        ),
        NodeKind::Container(child) => {
            let child_available = container_child_available(node, available);
            let (measured_child, _) = relayout_node(
                child,
                Point::new(
                    origin.x + node.style.padding.left,
                    origin.y + node.style.padding.top,
                ),
                child_available,
                text_system,
                retained,
                layout_dirty_roots,
                force_relayout || dirty,
            );
            let size = crate::layout::finalize_size(node, available, measured_child.frame.size);
            let measured = MeasuredNode {
                frame: zeno_core::Rect::new(origin.x, origin.y, size.width, size.height),
                kind: MeasuredKind::Single(Box::new(measured_child)),
            };
            (measured, false)
        }
        NodeKind::Stack { axis, children } => {
            let measured = relayout_stack(
                node,
                *axis,
                children,
                origin,
                available,
                text_system,
                retained,
                layout_dirty_roots,
                force_relayout || dirty,
            );
            (measured, false)
        }
    }
}

fn relayout_stack(
    node: &Node,
    axis: crate::Axis,
    children: &[Node],
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    retained: &RetainedComposeTree,
    layout_dirty_roots: &HashSet<NodeId>,
    force_relayout: bool,
) -> MeasuredNode {
    let mut measured_children = Vec::with_capacity(children.len());
    let content_origin = Point::new(
        origin.x + node.style.padding.left,
        origin.y + node.style.padding.top,
    );
    let content_available = Size::new(
        (available.width - node.style.padding.left - node.style.padding.right).max(0.0),
        (available.height - node.style.padding.top - node.style.padding.bottom).max(0.0),
    );
    let mut cursor = content_origin;
    let mut used_main = 0.0f32;
    let mut max_cross = 0.0f32;
    let spacing = node.style.spacing;
    let mut downstream_relayout = force_relayout;

    for child in children {
        let remaining = match axis {
            crate::Axis::Horizontal => {
                Size::new((content_available.width - used_main).max(0.0), content_available.height)
            }
            crate::Axis::Vertical => {
                Size::new(content_available.width, (content_available.height - used_main).max(0.0))
            }
        };
        let (measured_child, reused) = relayout_node(
            child,
            cursor,
            remaining,
            text_system,
            retained,
            layout_dirty_roots,
            downstream_relayout,
        );
        downstream_relayout = downstream_relayout || !reused;

        let child_size = measured_child.frame.size;
        match axis {
            crate::Axis::Horizontal => {
                used_main += child_size.width;
                if !measured_children.is_empty() {
                    used_main += spacing;
                }
                cursor.x += child_size.width + spacing;
                max_cross = max_cross.max(child_size.height);
            }
            crate::Axis::Vertical => {
                used_main += child_size.height;
                if !measured_children.is_empty() {
                    used_main += spacing;
                }
                cursor.y += child_size.height + spacing;
                max_cross = max_cross.max(child_size.width);
            }
        }
        measured_children.push(measured_child);
    }

    let content_size = match axis {
        crate::Axis::Horizontal => Size::new(used_main.max(0.0), max_cross),
        crate::Axis::Vertical => Size::new(max_cross, used_main.max(0.0)),
    };
    let final_size = crate::layout::finalize_size(node, available, content_size);
    MeasuredNode {
        frame: zeno_core::Rect::new(origin.x, origin.y, final_size.width, final_size.height),
        kind: MeasuredKind::Multiple(measured_children),
    }
}

fn collect_stack_fragments(
    node: &Node,
    children: &[Node],
    measured_children: &[MeasuredNode],
    available: Size,
    available_by_node: &mut HashMap<NodeId, Size>,
    fragments_by_node: &mut HashMap<NodeId, Vec<DrawCommand>>,
) {
    let content_available = container_child_available(node, available);
    let mut used_main = 0.0f32;
    for (index, (child, measured_child)) in children.iter().zip(measured_children.iter()).enumerate() {
        let child_available = match child_axis(node) {
            crate::Axis::Horizontal => {
                Size::new((content_available.width - used_main).max(0.0), content_available.height)
            }
            crate::Axis::Vertical => {
                Size::new(content_available.width, (content_available.height - used_main).max(0.0))
            }
        };
        collect_fragments(
            child,
            measured_child,
            child_available,
            available_by_node,
            fragments_by_node,
        );
        let child_size = measured_child.frame.size;
        match child_axis(node) {
            crate::Axis::Horizontal => {
                used_main += child_size.width;
            }
            crate::Axis::Vertical => {
                used_main += child_size.height;
            }
        }
        if index + 1 != children.len() {
            used_main += node.style.spacing;
        }
    }
}

fn container_child_available(node: &Node, available: Size) -> Size {
    Size::new(
        (available.width - node.style.padding.left - node.style.padding.right).max(0.0),
        (available.height - node.style.padding.top - node.style.padding.bottom).max(0.0),
    )
}

fn child_axis(node: &Node) -> crate::Axis {
    match &node.kind {
        NodeKind::Stack { axis, .. } => *axis,
        _ => crate::Axis::Vertical,
    }
}

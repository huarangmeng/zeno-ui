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
        if let Some(retained) = self.retained.as_ref() {
            if retained.can_reuse(root, viewport) {
                self.stats.cache_hits += 1;
                return SceneSubmit::Full(retained.scene().clone());
            }
        }

        if let Some(retained) = self.retained.as_mut() {
            if retained.dirty().requires_paint_only() && retained.can_repaint(root, viewport) {
                self.stats.compose_passes += 1;
                let dirty_node_ids = retained.dirty_node_ids();
                repaint_dirty_nodes(root, retained);
                let scene = retained.scene().clone();
                let patch = patch_for_nodes(&scene, &dirty_node_ids);
                return if patch.is_empty() {
                    SceneSubmit::Full(scene)
                } else {
                    SceneSubmit::Patch { patch, current: scene }
                };
            }
        }

        if let Some(retained) = self.retained.as_mut() {
            if retained.dirty().requires_layout() && retained.can_repaint(root, viewport) {
                self.stats.compose_passes += 1;
                self.stats.layout_passes += 1;
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
                return SceneSubmit::Full(scene);
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

fn patch_for_nodes(scene: &Scene, node_ids: &[NodeId]) -> ScenePatch {
    let upserts = scene
        .blocks
        .iter()
        .filter(|block| node_ids.iter().any(|node_id| node_id.0 == block.node_id))
        .cloned()
        .collect();
    ScenePatch {
        size: scene.size,
        base_block_count: scene.blocks.len(),
        upserts,
        removes: Vec::new(),
    }
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

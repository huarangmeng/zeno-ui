use std::collections::{HashMap, HashSet};

use zeno_core::{Point, Rect, Size, Transform2D};
use zeno_graphics::{
    Brush, DrawCommand, Scene, SceneBlock, SceneClip, ScenePatch, SceneSubmit, SceneTransform,
    Shape,
};
use zeno_text::TextSystem;

use crate::{
    invalidation::DirtyReason,
    layout::{measure_node, MeasuredKind, MeasuredNode},
    modifier::{ClipMode, TransformOrigin},
    tree::RetainedComposeTree,
    Node, NodeId, NodeKind,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ComposeStats {
    pub compose_passes: usize,
    pub layout_passes: usize,
    pub cache_hits: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelayoutClass {
    Reused,
    LocalOnly,
    ParentOnly,
    ParentAndFollowingSiblings,
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
                let dirty_node_ids: HashSet<NodeId> = retained.dirty_node_ids().into_iter().collect();
                repaint_dirty_nodes(root, retained);
                let patch = patch_scene_for_nodes(root, retained, &dirty_node_ids);
                let scene = retained.scene().clone();
                retained.sync_root(root.clone());
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
                let previous_dirty = retained.dirty();
                let dirty_node_ids: HashSet<NodeId> = retained.dirty_node_ids().into_iter().collect();
                let previous_node_ids: HashSet<NodeId> =
                    retained.available_map().keys().copied().collect();
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
                let available_by_node = available_map_from_measured(root, viewport, &measured);
                let current_node_ids: HashSet<NodeId> = available_by_node.keys().copied().collect();
                let new_node_ids: HashSet<NodeId> = current_node_ids
                    .difference(&previous_node_ids)
                    .copied()
                    .collect();
                let fragment_update_ids: HashSet<NodeId> = dirty_node_ids
                    .union(&new_node_ids)
                    .copied()
                    .collect();
                retained.apply_layout_state(root.clone(), viewport, measured.clone(), available_by_node);
                update_fragments_for_nodes(
                    root,
                    &measured,
                    viewport,
                    &fragment_update_ids,
                    retained,
                );
                let structure_changed = previous_dirty.requires_structure_rebuild()
                    || current_node_ids != previous_node_ids;
                if structure_changed {
                    retained.rebuild_scene_from_fragments();
                    let scene = retained.scene().clone();
                    return SceneSubmit::Full(scene);
                }
                let patch = patch_scene_for_nodes(root, retained, &current_node_ids);
                let scene = retained.scene().clone();
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

fn available_map_from_measured(
    root: &Node,
    viewport: Size,
    measured: &MeasuredNode,
) -> HashMap<NodeId, Size> {
    let mut available_by_node = HashMap::new();
    collect_available(root, measured, viewport, &mut available_by_node);
    available_by_node
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

fn collect_available(
    node: &Node,
    measured: &MeasuredNode,
    available: Size,
    available_by_node: &mut HashMap<NodeId, Size>,
) {
    available_by_node.insert(node.id(), available);
    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            collect_available(
                child,
                measured_child,
                container_child_available(node, available),
                available_by_node,
            );
        }
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
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
                collect_available(child, measured_child, child_available, available_by_node);
                used_main += main_axis_extent(measured_child.frame.size, child_axis(node));
                if index + 1 != children.len() {
                    used_main += node.resolved_style().spacing;
                }
            }
        }
        _ => {}
    }
}

fn node_fragment(node: &Node, measured: &MeasuredNode) -> Vec<DrawCommand> {
    let style = node.resolved_style();
    let mut fragment = Vec::new();
    let local_bounds = Rect::new(0.0, 0.0, measured.frame.size.width, measured.frame.size.height);
    if let Some(background) = style.background {
        let shape = if style.corner_radius > 0.0 {
            Shape::RoundedRect {
                rect: local_bounds,
                radius: style.corner_radius,
            }
        } else {
            Shape::Rect(local_bounds)
        };
        fragment.push(DrawCommand::Fill {
            shape,
            brush: Brush::Solid(background),
        });
    }

    match (&node.kind, &measured.kind) {
        (NodeKind::Text(_), MeasuredKind::Text(layout)) => {
            let position = Point::new(
                style.padding.left,
                style.padding.top + layout.metrics.ascent,
            );
            fragment.push(DrawCommand::Text {
                position,
                layout: layout.clone(),
                color: style.foreground,
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
}

fn patch_scene_for_nodes(
    root: &Node,
    retained: &mut RetainedComposeTree,
    _update_ids: &HashSet<NodeId>,
) -> ScenePatch {
    let previous_scene = retained.scene().clone();
    let scene = build_scene(root, retained.measured(), retained.viewport(), retained.fragments());
    let patch = diff_scenes(&previous_scene, &scene);
    retained.replace_scene(scene);
    patch
}

fn update_fragments_for_nodes(
    node: &Node,
    measured: &MeasuredNode,
    available: Size,
    update_ids: &HashSet<NodeId>,
    retained: &mut RetainedComposeTree,
) -> bool {
    let mut touched = update_ids.contains(&node.id());
    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            touched |= update_fragments_for_nodes(
                child,
                measured_child,
                container_child_available(node, available),
                update_ids,
                retained,
            );
        }
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            let content_available = container_child_available(node, available);
            let mut used_main = 0.0f32;
            let axis = child_axis(node);
            for (index, (child, measured_child)) in children.iter().zip(measured_children.iter()).enumerate() {
                let child_available = match axis {
                    crate::Axis::Horizontal => {
                        Size::new((content_available.width - used_main).max(0.0), content_available.height)
                    }
                    crate::Axis::Vertical => {
                        Size::new(content_available.width, (content_available.height - used_main).max(0.0))
                    }
                };
                touched |= update_fragments_for_nodes(
                    child,
                    measured_child,
                    child_available,
                    update_ids,
                    retained,
                );
                used_main += main_axis_extent(measured_child.frame.size, axis);
                if index + 1 != children.len() {
                    used_main += node.resolved_style().spacing;
                }
            }
        }
        _ => {}
    }
    if touched && update_ids.contains(&node.id()) {
        retained.update_fragment(node.id(), node_fragment(node, measured));
    }
    touched
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
    let (layers, blocks) = build_layers_and_blocks(root, measured, fragments_by_node, viewport);
    Scene::from_layers_and_blocks(viewport, None, layers, blocks)
}

fn diff_scenes(previous: &Scene, current: &Scene) -> ScenePatch {
    let previous_layers_by_id: HashMap<u64, &zeno_graphics::SceneLayer> = previous
        .layers
        .iter()
        .map(|layer| (layer.layer_id, layer))
        .collect();
    let previous_blocks_by_id: HashMap<u64, &SceneBlock> = previous
        .blocks
        .iter()
        .map(|block| (block.node_id, block))
        .collect();
    let layer_upserts = current
        .layers
        .iter()
        .filter(|layer| layer.layer_id != Scene::ROOT_LAYER_ID)
        .filter(|layer| previous_layers_by_id.get(&layer.layer_id).copied() != Some(*layer))
        .cloned()
        .collect();
    let layer_removes = previous
        .layers
        .iter()
        .filter(|layer| layer.layer_id != Scene::ROOT_LAYER_ID)
        .filter(|layer| !current.layers.iter().any(|current_layer| current_layer.layer_id == layer.layer_id))
        .map(|layer| layer.layer_id)
        .collect();
    let upserts = current
        .blocks
        .iter()
        .filter(|block| previous_blocks_by_id.get(&block.node_id).copied() != Some(*block))
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
        base_layer_count: previous.layers.len(),
        base_block_count: previous.blocks.len(),
        layer_upserts,
        layer_removes,
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
    let mut current_by_id = HashMap::new();
    index_nodes(root, &mut current_by_id);
    if previous_by_id.len() != current_by_id.len() {
        retained.mark_dirty(DirtyReason::Structure);
        return;
    }
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
    let previous_style = previous.resolved_style();
    let current_style = current.resolved_style();
    if previous_style.padding != current_style.padding
        || previous_style.width != current_style.width
        || previous_style.height != current_style.height
        || (stack_node && previous_style.spacing != current_style.spacing)
    {
        return Some(DirtyReason::Layout);
    }
    if previous_style.background != current_style.background
        || previous_style.corner_radius != current_style.corner_radius
        || previous_style.clip != current_style.clip
        || previous_style.transform != current_style.transform
        || previous_style.transform_origin != current_style.transform_origin
        || previous_style.opacity != current_style.opacity
        || previous_style.layer != current_style.layer
        || (text_node && previous_style.foreground != current_style.foreground)
    {
        return Some(DirtyReason::Paint);
    }
    None
}

fn child_ids(children: &[Node]) -> Vec<NodeId> {
    children.iter().map(Node::id).collect()
}

fn build_layers_and_blocks(
    node: &Node,
    measured: &MeasuredNode,
    fragments_by_node: &HashMap<NodeId, Vec<DrawCommand>>,
    viewport: Size,
) -> (Vec<zeno_graphics::SceneLayer>, Vec<SceneBlock>) {
    let mut layers = vec![zeno_graphics::SceneLayer::root(viewport)];
    let mut blocks = Vec::new();
    let mut next_order = 1u32;
    collect_scene_items(
        node,
        measured,
        fragments_by_node,
        Scene::ROOT_LAYER_ID,
        Point::new(0.0, 0.0),
        Transform2D::identity(),
        &mut next_order,
        &mut layers,
        &mut blocks,
    );
    (layers, blocks)
}

fn collect_scene_items(
    node: &Node,
    measured: &MeasuredNode,
    fragments_by_node: &HashMap<NodeId, Vec<DrawCommand>>,
    current_layer_id: u64,
    current_layer_origin: Point,
    current_layer_world_transform: Transform2D,
    next_order: &mut u32,
    layers: &mut Vec<zeno_graphics::SceneLayer>,
    blocks: &mut Vec<SceneBlock>,
) {
    let style = node.resolved_style();
    let local_bounds = Rect::new(0.0, 0.0, measured.frame.size.width, measured.frame.size.height);
    if node_creates_layer(&style) {
        let layer_transform = layer_local_transform(
            measured.frame.origin,
            current_layer_origin,
            measured.frame.size,
            style.transform,
            style.transform_origin,
        );
        let world_transform = current_layer_world_transform.then(layer_transform);
        let layer_id = node.id().0;
        let order = *next_order;
        *next_order += 1;
        layers.push(zeno_graphics::SceneLayer::new(
            layer_id,
            node.id().0,
            Some(current_layer_id),
            order,
            local_bounds,
            world_transform.map_rect(local_bounds),
            layer_transform,
            scene_clip(measured.frame.size, style.clip),
            style.opacity,
            style.layer || style.opacity < 1.0,
        ));
        if let Some(fragment) = fragments_by_node.get(&node.id()) {
            blocks.push(SceneBlock::new(
                node.id().0,
                layer_id,
                *next_order,
                world_transform.map_rect(local_bounds),
                Transform2D::identity(),
                None,
                fragment.clone(),
            ));
            *next_order += 1;
        }
        collect_scene_children(
            node,
            measured,
            fragments_by_node,
            layer_id,
            measured.frame.origin,
            world_transform,
            next_order,
            layers,
            blocks,
        );
        return;
    }

    if let Some(fragment) = fragments_by_node.get(&node.id()) {
        let block_transform =
            Transform2D::translation(measured.frame.origin.x - current_layer_origin.x, measured.frame.origin.y - current_layer_origin.y);
        let world_transform = current_layer_world_transform.then(block_transform);
        blocks.push(SceneBlock::new(
            node.id().0,
            current_layer_id,
            *next_order,
            world_transform.map_rect(local_bounds),
            block_transform,
            None,
            fragment.clone(),
        ));
        *next_order += 1;
    }
    collect_scene_children(
        node,
        measured,
        fragments_by_node,
        current_layer_id,
        current_layer_origin,
        current_layer_world_transform,
        next_order,
        layers,
        blocks,
    );
}

fn collect_scene_children(
    node: &Node,
    measured: &MeasuredNode,
    fragments_by_node: &HashMap<NodeId, Vec<DrawCommand>>,
    current_layer_id: u64,
    current_layer_origin: Point,
    current_layer_world_transform: Transform2D,
    next_order: &mut u32,
    layers: &mut Vec<zeno_graphics::SceneLayer>,
    blocks: &mut Vec<SceneBlock>,
) {
    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            collect_scene_items(
                child,
                measured_child,
                fragments_by_node,
                current_layer_id,
                current_layer_origin,
                current_layer_world_transform,
                next_order,
                layers,
                blocks,
            );
        }
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            for (child, measured_child) in children.iter().zip(measured_children.iter()) {
                collect_scene_items(
                    child,
                    measured_child,
                    fragments_by_node,
                    current_layer_id,
                    current_layer_origin,
                    current_layer_world_transform,
                    next_order,
                    layers,
                    blocks,
                );
            }
        }
        _ => {}
    }
}

fn node_creates_layer(style: &crate::Style) -> bool {
    style.layer || style.opacity < 1.0 || style.clip.is_some() || !style.transform.is_identity()
}

fn layer_local_transform(
    node_origin: Point,
    parent_layer_origin: Point,
    size: Size,
    local_transform: Transform2D,
    transform_origin: TransformOrigin,
) -> SceneTransform {
    let pivot = Point::new(size.width * transform_origin.x, size.height * transform_origin.y);
    Transform2D::translation(-pivot.x, -pivot.y)
        .then(local_transform)
        .then(Transform2D::translation(pivot.x, pivot.y))
        .then(Transform2D::translation(
            node_origin.x - parent_layer_origin.x,
            node_origin.y - parent_layer_origin.y,
        ))
}

fn scene_clip(size: Size, clip: Option<ClipMode>) -> Option<SceneClip> {
    match clip {
        Some(ClipMode::Bounds) => Some(SceneClip::Rect(Rect::new(0.0, 0.0, size.width, size.height))),
        Some(ClipMode::RoundedBounds { radius }) => Some(SceneClip::RoundedRect {
            rect: Rect::new(0.0, 0.0, size.width, size.height),
            radius,
        }),
        None => None,
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
    let descendant_dirty = retained.has_descendant_in(node.id(), layout_dirty_roots);
    if !force_relayout && !dirty && !descendant_dirty {
        if let (Some(measured), Some(cached_available)) =
            (retained.measured_for(node.id()), retained.available_for(node.id()))
        {
            if cached_available == available && measured.frame.origin == origin {
                return (measured.clone(), true);
            }
        }
    }

    match &node.kind {
        NodeKind::Text(text) => {
            let measured = crate::layout::measure_text(node, text, origin, available, text_system);
            let _ = classify_leaf_relayout(retained.measured_for(node.id()), &measured);
            (measured, false)
        }
        NodeKind::Spacer(spacer) => {
            let measured = crate::layout::measure_spacer(node, spacer, origin, available);
            let _ = classify_leaf_relayout(retained.measured_for(node.id()), &measured);
            (measured, false)
        }
        NodeKind::Container(child) => {
            let style = node.resolved_style();
            let previous_measured = retained.measured_for(node.id());
            let previous_child = previous_measured.and_then(|measured| match &measured.kind {
                MeasuredKind::Single(child) => Some(child.as_ref()),
                _ => None,
            });
            let child_available = container_child_available(node, available);
            let (measured_child, child_reused) = relayout_node(
                child,
                Point::new(
                    origin.x + style.padding.left,
                    origin.y + style.padding.top,
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
            let _ = classify_container_relayout(previous_measured, previous_child, &measured);
            let _ = child_reused;
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
    let style = node.resolved_style();
    let mut measured_children = Vec::with_capacity(children.len());
    let content_origin = Point::new(
        origin.x + style.padding.left,
        origin.y + style.padding.top,
    );
    let content_available = Size::new(
        (available.width - style.padding.left - style.padding.right).max(0.0),
        (available.height - style.padding.top - style.padding.bottom).max(0.0),
    );
    let mut cursor = content_origin;
    let mut used_main = 0.0f32;
    let mut max_cross = 0.0f32;
    let spacing = style.spacing;
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
        if !downstream_relayout && !reused {
            let previous_measured = retained.measured_for(child.id());
            let child_class = classify_stack_child_relayout(previous_measured, &measured_child, axis);
            downstream_relayout = matches!(child_class, RelayoutClass::ParentAndFollowingSiblings);
        }

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
            used_main += node.resolved_style().spacing;
        }
    }
}

fn container_child_available(node: &Node, available: Size) -> Size {
    let style = node.resolved_style();
    Size::new(
        (available.width - style.padding.left - style.padding.right).max(0.0),
        (available.height - style.padding.top - style.padding.bottom).max(0.0),
    )
}

fn child_axis(node: &Node) -> crate::Axis {
    match &node.kind {
        NodeKind::Stack { axis, .. } => *axis,
        _ => crate::Axis::Vertical,
    }
}

fn main_axis_extent(size: Size, axis: crate::Axis) -> f32 {
    match axis {
        crate::Axis::Horizontal => size.width,
        crate::Axis::Vertical => size.height,
    }
}

fn classify_leaf_relayout(previous: Option<&MeasuredNode>, current: &MeasuredNode) -> RelayoutClass {
    match previous {
        Some(previous) if previous.frame == current.frame => RelayoutClass::LocalOnly,
        Some(_) => RelayoutClass::ParentOnly,
        None => RelayoutClass::ParentOnly,
    }
}

fn classify_container_relayout(
    previous: Option<&MeasuredNode>,
    previous_child: Option<&MeasuredNode>,
    current: &MeasuredNode,
) -> RelayoutClass {
    let Some(previous) = previous else {
        return RelayoutClass::ParentOnly;
    };
    if previous.frame != current.frame {
        return RelayoutClass::ParentOnly;
    }
    let current_child = match &current.kind {
        MeasuredKind::Single(child) => Some(child.as_ref()),
        _ => None,
    };
    if previous_child == current_child {
        RelayoutClass::Reused
    } else {
        RelayoutClass::LocalOnly
    }
}

fn classify_stack_child_relayout(
    previous: Option<&MeasuredNode>,
    current: &MeasuredNode,
    axis: crate::Axis,
) -> RelayoutClass {
    let Some(previous) = previous else {
        return RelayoutClass::ParentAndFollowingSiblings;
    };
    let previous_main = main_axis_extent(previous.frame.size, axis);
    let current_main = main_axis_extent(current.frame.size, axis);
    if (previous_main - current_main).abs() > f32::EPSILON {
        RelayoutClass::ParentAndFollowingSiblings
    } else if previous.frame == current.frame {
        RelayoutClass::LocalOnly
    } else {
        RelayoutClass::ParentOnly
    }
}

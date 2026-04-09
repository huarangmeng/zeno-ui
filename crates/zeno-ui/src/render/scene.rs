//! scene 构建相关逻辑集中在这里，便于继续演进 retained scene 结构。

use super::*;

pub(super) fn build_scene(
    root: &Node,
    measured: &MeasuredNode,
    viewport: Size,
    fragments_by_node: &HashMap<NodeId, Vec<DrawCommand>>,
) -> Scene {
    let (layers, blocks) = build_layers_and_blocks(root, measured, fragments_by_node, viewport);
    Scene::from_layers_and_blocks(viewport, None, layers, blocks)
}

pub(super) fn build_layers_and_blocks(
    root: &Node,
    measured: &MeasuredNode,
    fragments_by_node: &HashMap<NodeId, Vec<DrawCommand>>,
    viewport: Size,
) -> (Vec<zeno_scene::SceneLayer>, Vec<SceneBlock>) {
    let mut layers = vec![zeno_scene::SceneLayer::root(viewport)];
    let mut blocks = Vec::new();
    let mut next_order = 1u32;
    collect_scene_items(
        root,
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

pub(super) fn collect_scene_items(
    node: &Node,
    measured: &MeasuredNode,
    fragments_by_node: &HashMap<NodeId, Vec<DrawCommand>>,
    current_layer_id: u64,
    current_layer_origin: Point,
    current_layer_world_transform: Transform2D,
    next_order: &mut u32,
    layers: &mut Vec<zeno_scene::SceneLayer>,
    blocks: &mut Vec<SceneBlock>,
) {
    let style = node.resolved_style();
    let local_bounds = Rect::new(
        0.0,
        0.0,
        measured.frame.size.width,
        measured.frame.size.height,
    );
    if node_creates_layer(&style) {
        let layer_transform = layer_local_transform(
            measured.frame.origin,
            current_layer_origin,
            measured.frame.size,
            style.transform,
            style.transform_origin,
        );
        let world_transform = current_layer_world_transform.then(layer_transform);
        let effect_bounds = scene_effect_bounds(local_bounds, &style);
        let layer_id = node.id().0;
        let order = *next_order;
        *next_order += 1;
        layers.push(zeno_scene::SceneLayer::new(
            layer_id,
            node.id().0,
            Some(current_layer_id),
            order,
            local_bounds,
            world_transform.map_rect(effect_bounds),
            layer_transform,
            scene_clip(measured.frame.size, style.clip),
            style.opacity,
            scene_blend_mode(style.blend_mode),
            scene_effects(&style),
            style.layer
                || style.opacity < 1.0
                || style.blend_mode != BlendMode::Normal
                || style.blur.is_some()
                || style.drop_shadow.is_some(),
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
        let block_transform = Transform2D::translation(
            measured.frame.origin.x - current_layer_origin.x,
            measured.frame.origin.y - current_layer_origin.y,
        );
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

pub(super) fn collect_scene_children(
    node: &Node,
    measured: &MeasuredNode,
    fragments_by_node: &HashMap<NodeId, Vec<DrawCommand>>,
    current_layer_id: u64,
    current_layer_origin: Point,
    current_layer_world_transform: Transform2D,
    next_order: &mut u32,
    layers: &mut Vec<zeno_scene::SceneLayer>,
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

pub(super) fn node_creates_layer(style: &crate::Style) -> bool {
    style.layer
        || style.opacity < 1.0
        || style.clip.is_some()
        || !style.transform.is_identity()
        || style.blend_mode != BlendMode::Normal
        || style.blur.is_some()
        || style.drop_shadow.is_some()
}

pub(super) fn layer_local_transform(
    node_origin: Point,
    parent_layer_origin: Point,
    size: Size,
    local_transform: Transform2D,
    transform_origin: TransformOrigin,
) -> SceneTransform {
    let pivot = Point::new(
        size.width * transform_origin.x,
        size.height * transform_origin.y,
    );
    Transform2D::translation(-pivot.x, -pivot.y)
        .then(local_transform)
        .then(Transform2D::translation(pivot.x, pivot.y))
        .then(Transform2D::translation(
            node_origin.x - parent_layer_origin.x,
            node_origin.y - parent_layer_origin.y,
        ))
}

pub(super) fn scene_clip(size: Size, clip: Option<ClipMode>) -> Option<SceneClip> {
    match clip {
        Some(ClipMode::Bounds) => Some(SceneClip::Rect(Rect::new(
            0.0,
            0.0,
            size.width,
            size.height,
        ))),
        Some(ClipMode::RoundedBounds { radius }) => Some(SceneClip::RoundedRect {
            rect: Rect::new(0.0, 0.0, size.width, size.height),
            radius,
        }),
        None => None,
    }
}

pub(super) fn scene_blend_mode(mode: BlendMode) -> SceneBlendMode {
    match mode {
        BlendMode::Normal => SceneBlendMode::Normal,
        BlendMode::Multiply => SceneBlendMode::Multiply,
        BlendMode::Screen => SceneBlendMode::Screen,
    }
}

pub(super) fn scene_effects(style: &crate::Style) -> Vec<SceneEffect> {
    let mut effects = Vec::new();
    if let Some(sigma) = style.blur {
        effects.push(SceneEffect::Blur { sigma });
    }
    if let Some(shadow) = style.drop_shadow {
        effects.push(SceneEffect::DropShadow {
            dx: shadow.dx,
            dy: shadow.dy,
            blur: shadow.blur,
            color: shadow.color,
        });
    }
    effects
}

pub(super) fn scene_effect_bounds(local_bounds: Rect, style: &crate::Style) -> Rect {
    let mut bounds = local_bounds;
    if let Some(sigma) = style.blur {
        bounds = expand_rect(bounds, sigma * 3.0);
    }
    if let Some(shadow) = style.drop_shadow {
        let shadow_bounds = expand_rect(
            Rect::new(
                bounds.origin.x + shadow.dx,
                bounds.origin.y + shadow.dy,
                bounds.size.width,
                bounds.size.height,
            ),
            shadow.blur * 3.0,
        );
        bounds = bounds.union(&shadow_bounds);
    }
    bounds
}

fn expand_rect(rect: Rect, amount: f32) -> Rect {
    Rect::new(
        rect.origin.x - amount,
        rect.origin.y - amount,
        rect.size.width + amount * 2.0,
        rect.size.height + amount * 2.0,
    )
}

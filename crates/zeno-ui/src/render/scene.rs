//! scene 构建相关逻辑集中在这里，便于继续演进 retained scene 结构。

use super::*;
use crate::frontend::{FrontendObject, FrontendObjectTable, compile_object_table};
use crate::layout::LayoutArena;

pub(super) fn build_scene(
    root: &Node,
    layout: &LayoutArena,
    viewport: Size,
    fragments: &crate::render::fragments::FragmentStore,
) -> Scene {
    let frontend = compile_object_table(root);
    let (layers, objects) = build_layers_and_objects(&frontend, layout, fragments, viewport);
    Scene::from_layers_and_objects(viewport, None, layers, objects)
}

pub(super) fn build_layers_and_objects(
    frontend: &FrontendObjectTable,
    layout: &LayoutArena,
    fragments: &crate::render::fragments::FragmentStore,
    viewport: Size,
) -> (Vec<LayerObject>, Vec<RenderObject>) {
    let mut layers = vec![LayerObject::root(viewport)];
    let mut objects = Vec::new();
    let mut next_order = 1u32;
    let mut stack = vec![SceneVisit {
        index: 0,
        current_layer_id: Scene::ROOT_LAYER_ID,
        current_layer_origin: Point::new(0.0, 0.0),
        current_layer_world_transform: Transform2D::identity(),
    }];
    while let Some(visit) = stack.pop() {
        let scene_node = scene_item_from_object(
            frontend.object(visit.index),
            visit.index,
            layout,
            fragments,
            visit.current_layer_id,
            visit.current_layer_origin,
            visit.current_layer_world_transform,
            next_order,
        );
        next_order = scene_node.next_order;
        if let Some(layer) = scene_node.layer {
            let next_layer_id = layer.layer_id;
            let next_origin = layout.slot_at(visit.index).frame.origin;
            let next_transform = scene_node.next_world_transform;
            layers.push(layer);
            if let Some(object) = scene_node.object {
                objects.push(object);
            }
            for &child_index in frontend.child_indices(visit.index).iter().rev() {
                stack.push(SceneVisit {
                    index: child_index,
                    current_layer_id: next_layer_id,
                    current_layer_origin: next_origin,
                    current_layer_world_transform: next_transform,
                });
            }
        } else {
            if let Some(object) = scene_node.object {
                objects.push(object);
            }
            for &child_index in frontend.child_indices(visit.index).iter().rev() {
                stack.push(SceneVisit {
                    index: child_index,
                    current_layer_id: visit.current_layer_id,
                    current_layer_origin: visit.current_layer_origin,
                    current_layer_world_transform: visit.current_layer_world_transform,
                });
            }
        }
    }
    (layers, objects)
}

#[derive(Clone, Copy)]
struct SceneVisit {
    index: usize,
    current_layer_id: u64,
    current_layer_origin: Point,
    current_layer_world_transform: Transform2D,
}

struct SceneItemResult {
    layer: Option<LayerObject>,
    object: Option<RenderObject>,
    next_world_transform: Transform2D,
    next_order: u32,
}

fn scene_item_from_object(
    object: &FrontendObject,
    index: usize,
    layout: &LayoutArena,
    fragments: &crate::render::fragments::FragmentStore,
    current_layer_id: u64,
    current_layer_origin: Point,
    current_layer_world_transform: Transform2D,
    next_order: u32,
) -> SceneItemResult {
    let slot = layout.slot_at(index);
    let style = &object.style;
    let local_bounds = Rect::new(
        0.0,
        0.0,
        slot.frame.size.width,
        slot.frame.size.height,
    );
    if node_creates_layer(&style) {
        let layer_transform = layer_local_transform(
            slot.frame.origin,
            current_layer_origin,
            slot.frame.size,
            style.transform,
            style.transform_origin,
        );
        let world_transform = current_layer_world_transform.then(layer_transform);
        let effect_bounds = scene_effect_bounds(local_bounds, &style);
        let layer_id = object.node_id.0;
        let order = next_order;
        let layer = LayerObject::new(
            layer_id,
            object.node_id.0,
            Some(current_layer_id),
            order,
            local_bounds,
            world_transform.map_rect(effect_bounds),
            layer_transform,
            scene_clip(slot.frame.size, style.clip),
            style.opacity,
            scene_blend_mode(style.blend_mode),
            scene_effects(&style),
            style.layer
                || style.opacity < 1.0
                || style.blend_mode != BlendMode::Normal
                || style.blur.is_some()
                || style.drop_shadow.is_some(),
        );
        let mut next = order + 1;
        let scene_object = fragments.clone_fragment_at(index).map(|fragment| {
            let object = RenderObject::new(
                object.node_id.0,
                layer_id,
                next,
                world_transform.map_rect(local_bounds),
                Transform2D::identity(),
                None,
                fragment,
            );
            next += 1;
            object
        });
        return SceneItemResult {
            layer: Some(layer),
            object: scene_object,
            next_world_transform: world_transform,
            next_order: next,
        };
    }

    let scene_object = fragments.clone_fragment_at(index).map(|fragment| {
        let block_transform = Transform2D::translation(
            slot.frame.origin.x - current_layer_origin.x,
            slot.frame.origin.y - current_layer_origin.y,
        );
        let world_transform = current_layer_world_transform.then(block_transform);
        RenderObject::new(
            object.node_id.0,
            current_layer_id,
            next_order,
            world_transform.map_rect(local_bounds),
            block_transform,
            None,
            fragment,
        )
    });
    let advanced_order = next_order + usize::from(scene_object.is_some()) as u32;
    SceneItemResult {
        layer: None,
        object: scene_object,
        next_world_transform: current_layer_world_transform,
        next_order: advanced_order,
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

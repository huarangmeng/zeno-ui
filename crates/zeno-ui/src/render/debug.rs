//! 调试输出与一次性 compose helper 放在这里，避免和增量路径纠缠。

use super::fragments::structured_scene_from_layout;
use super::*;
use crate::layout::{MeasuredKind, MeasuredNode, measure_node};

pub(super) fn compose_scene_internal(
    root: &Node,
    viewport: Size,
    text_system: &dyn TextSystem,
) -> Scene {
    let measured = measure_node(root, Point::new(0.0, 0.0), viewport, text_system);
    let layout = crate::layout::LayoutArena::from_measured(root, &measured);
    structured_scene_from_layout(root, viewport, &layout).2
}

pub(super) fn dump_scene(scene: &Scene) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "scene size=({:.1}, {:.1}) clear={:?} layers={} blocks={}",
        scene.size.width,
        scene.size.height,
        scene.clear_color,
        scene.layers.len(),
        scene.blocks.len()
    );
    for layer in &scene.layers {
        let _ = writeln!(
            output,
            "layer id={} node={} parent={:?} order={} opacity={:.2} blend={:?} effects={:?} offscreen={} bounds={:?}",
            layer.layer_id,
            layer.node_id,
            layer.parent_layer_id,
            layer.order,
            layer.opacity,
            layer.blend_mode,
            layer.effects,
            layer.offscreen,
            layer.bounds
        );
    }
    for block in &scene.blocks {
        let _ = writeln!(
            output,
            "block node={} layer={} order={} bounds={:?} clip={:?} commands={} resources={}",
            block.node_id,
            block.layer_id,
            block.order,
            block.bounds,
            block.clip,
            block.command_count,
            block.resource_keys.len()
        );
    }
    output
}

pub(super) fn dump_layout(root: &Node, viewport: Size, text_system: &dyn TextSystem) -> String {
    let measured = measure_node(root, Point::new(0.0, 0.0), viewport, text_system);
    let mut output = String::new();
    dump_layout_node(root, &measured, 0, &mut output);
    output
}

fn dump_layout_node(node: &Node, measured: &MeasuredNode, depth: usize, output: &mut String) {
    let indent = "  ".repeat(depth);
    let kind = match (&node.kind, &measured.kind) {
        (NodeKind::Text(_), MeasuredKind::Text(layout)) => format!(
            "text lines={} ascent={:.1} descent={:.1}",
            layout.metrics.line_count, layout.metrics.ascent, layout.metrics.descent
        ),
        (NodeKind::Container(_), MeasuredKind::Single(_)) => "container".to_string(),
        (NodeKind::Box { .. }, MeasuredKind::Multiple(children)) => {
            format!("box children={}", children.len())
        }
        (NodeKind::Stack { axis, .. }, MeasuredKind::Multiple(children)) => {
            format!("stack axis={:?} children={}", axis, children.len())
        }
        (NodeKind::Spacer(_), MeasuredKind::Spacer) => "spacer".to_string(),
        _ => "unknown".to_string(),
    };
    let _ = writeln!(
        output,
        "{}node id={} frame={:?} {}",
        indent,
        node.id().0,
        measured.frame,
        kind
    );
    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            dump_layout_node(child.as_ref(), measured_child, depth + 1, output);
        }
        (NodeKind::Box { children }, MeasuredKind::Multiple(measured_children))
        | (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            for (child, measured_child) in children.iter().zip(measured_children.iter()) {
                dump_layout_node(child, measured_child, depth + 1, output);
            }
        }
        _ => {}
    }
}

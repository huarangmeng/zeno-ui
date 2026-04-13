use fontdue::Font;
use metal::{CommandQueue, Device, MTLScissorRect, RenderPipelineState};
use zeno_core::Transform2D;
use zeno_scene::{DrawOp, RetainedScene, Scene};
use zeno_text::GlyphRasterCache;

use super::{
    draw::draw_commands,
    offscreen::{render_offscreen_layer_retained, should_render_offscreen},
    scissor::{clip_rect, intersect_scissor, scissor_for_rect},
};

#[allow(clippy::too_many_arguments)]
pub(super) fn render_retained_scene_layers(
    device: &Device,
    queue: &CommandQueue,
    color_pipeline: &RenderPipelineState,
    text_pipeline: &RenderPipelineState,
    composite_pipeline: &RenderPipelineState,
    composite_multiply_pipeline: &RenderPipelineState,
    composite_screen_pipeline: &RenderPipelineState,
    font: Option<&Font>,
    encoder: &metal::RenderCommandEncoderRef,
    scene: &mut RetainedScene,
    root_scissor: MTLScissorRect,
    viewport_width: f32,
    viewport_height: f32,
    glyph_cache: &GlyphRasterCache,
) {
    // RetainedScene caches traversal order; clone ops to avoid borrow conflicts while drawing.
    let ops = scene.draw_ops().to_vec();
    let mut layer_stack: Vec<LayerState> = Vec::new();
    let mut i = 0usize;
    while i < ops.len() {
        match ops[i] {
            DrawOp::EnterLayer(layer_index) => {
                let layer = scene.layer(layer_index);
                // Root layer is always rendered in-place.
                if layer.layer_id != Scene::ROOT_LAYER_ID && should_render_offscreen(layer) {
                    let parent = layer_stack.last().copied().unwrap_or(LayerState {
                        transform: Transform2D::identity(),
                        opacity: 1.0,
                        scissor: root_scissor,
                    });
                    let combined_transform = parent.transform.then(layer.transform);
                    let combined_opacity = parent.opacity * layer.opacity;
                    render_offscreen_layer_retained(
                        device,
                        queue,
                        color_pipeline,
                        text_pipeline,
                        composite_pipeline,
                        composite_multiply_pipeline,
                        composite_screen_pipeline,
                        font,
                        encoder,
                        scene,
                        &ops,
                        i,
                        layer_index,
                        combined_transform,
                        combined_opacity,
                        parent.scissor,
                        viewport_width,
                        viewport_height,
                        glyph_cache,
                    );
                    // Skip the entire subtree; offscreen renderer already handled it.
                    i = skip_subtree(&ops, i);
                    encoder.set_scissor_rect(parent.scissor);
                    continue;
                }

                let parent = layer_stack.last().copied().unwrap_or(LayerState {
                    transform: Transform2D::identity(),
                    opacity: 1.0,
                    scissor: root_scissor,
                });
                let combined_transform = if layer.layer_id == Scene::ROOT_LAYER_ID {
                    parent.transform
                } else {
                    parent.transform.then(layer.transform)
                };
                let layer_scissor = layer.clip.map_or(parent.scissor, |clip| {
                    intersect_scissor(
                        parent.scissor,
                        scissor_for_rect(
                            clip_rect(clip, combined_transform),
                            viewport_width,
                            viewport_height,
                        ),
                    )
                });
                encoder.set_scissor_rect(layer_scissor);
                layer_stack.push(LayerState {
                    transform: combined_transform,
                    opacity: parent.opacity * layer.opacity,
                    scissor: layer_scissor,
                });
                i += 1;
            }
            DrawOp::DrawObject(object_index) => {
                let Some(state) = layer_stack.last().copied() else {
                    i += 1;
                    continue;
                };
                let object = scene.object(object_index);
                let object_transform = state.transform.then(object.transform);
                let object_scissor = object.clip.map_or(state.scissor, |clip| {
                    intersect_scissor(
                        state.scissor,
                        scissor_for_rect(
                            clip_rect(clip, object_transform),
                            viewport_width,
                            viewport_height,
                        ),
                    )
                });
                encoder.set_scissor_rect(object_scissor);
                draw_commands(
                    device,
                    color_pipeline,
                    text_pipeline,
                    font,
                    encoder,
                    scene.packets_for_object_index(object_index),
                    viewport_width,
                    viewport_height,
                    object_transform,
                    state.opacity,
                    glyph_cache,
                );
                encoder.set_scissor_rect(state.scissor);
                i += 1;
            }
            DrawOp::ExitLayer(_) => {
                let _ = layer_stack.pop();
                let scissor = layer_stack
                    .last()
                    .copied()
                    .map(|s| s.scissor)
                    .unwrap_or(root_scissor);
                encoder.set_scissor_rect(scissor);
                i += 1;
            }
        }
    }
    encoder.set_scissor_rect(root_scissor);
}

#[derive(Clone, Copy)]
struct LayerState {
    transform: Transform2D,
    opacity: f32,
    scissor: MTLScissorRect,
}

fn skip_subtree(ops: &[DrawOp], enter_index: usize) -> usize {
    let mut depth = 0i32;
    let mut i = enter_index;
    while i < ops.len() {
        match ops[i] {
            DrawOp::EnterLayer(_) => depth += 1,
            DrawOp::ExitLayer(_) => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    ops.len()
}

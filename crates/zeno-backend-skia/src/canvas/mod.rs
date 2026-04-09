mod draw;
mod effects;
mod layer;
mod mapping;
mod text;

use skia_safe as sk;
use zeno_core::{Color, Rect};
use zeno_scene::{DrawCommand, Scene};

pub use text::{SkiaTextCache, SkiaTextCacheStats};

use draw::draw_command;
use layer::render_scene_layers;
use mapping::sk_color;

pub fn render_scene_to_canvas(canvas: &sk::Canvas, scene: &Scene, text_cache: &mut SkiaTextCache) {
    if let Some(clear_color) = scene.clear_color {
        canvas.clear(sk_color(clear_color));
    }
    if scene.blocks.is_empty() {
        for cmd in scene.iter_commands() {
            draw_command(canvas, cmd, text_cache);
        }
        return;
    }
    render_scene_layers(canvas, scene, text_cache);
}

pub fn render_scene_region_to_canvas(
    canvas: &sk::Canvas,
    scene: &Scene,
    dirty_rect: Rect,
    text_cache: &mut SkiaTextCache,
) {
    let clip = sk::Rect::from_xywh(
        dirty_rect.origin.x,
        dirty_rect.origin.y,
        dirty_rect.size.width,
        dirty_rect.size.height,
    );

    // 局部重绘时先裁剪并按场景背景清空脏区，避免旧像素残留到新的 layer 合成结果里。
    canvas.save();
    canvas.clip_rect(clip, None, Some(false));
    canvas.draw_rect(clip, &clear_paint(scene));
    render_scene_layers(canvas, scene, text_cache);
    canvas.restore();
}

fn clear_paint(scene: &Scene) -> sk::Paint {
    let mut paint = sk::Paint::default();
    paint.set_style(sk::paint::Style::Fill);
    paint.set_anti_alias(true);
    let clear = scene
        .clear_color
        .or_else(|| scene.clear_command())
        .unwrap_or(Color::TRANSPARENT);
    paint.set_color(sk_color(clear));
    paint
}

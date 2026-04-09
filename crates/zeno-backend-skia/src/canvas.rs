use std::collections::HashMap;

use skia_safe as sk;
use zeno_core::{Color, Rect, Transform2D};
use zeno_graphics::{
    DrawCommand, Scene, SceneBlendMode, SceneBlock, SceneClip, SceneEffect, SceneLayer,
    SceneResourceKey, Shape,
};

#[derive(Default)]
pub struct SkiaTextCache {
    typefaces: HashMap<SceneResourceKey, Option<sk::Typeface>>,
    fonts: HashMap<SceneResourceKey, sk::Font>,
    stats: SkiaTextCacheStats,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SkiaTextCacheStats {
    pub typeface_hits: usize,
    pub font_hits: usize,
    pub cached_typefaces: usize,
    pub cached_fonts: usize,
}

pub fn render_scene_to_canvas(canvas: &sk::Canvas, scene: &Scene, text_cache: &mut SkiaTextCache) {
    if let Some(clear_color) = scene.clear_color {
        canvas.clear(sk_color(clear_color));
    }
    if scene.blocks.is_empty() {
        for cmd in &scene.commands {
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
    canvas.save();
    canvas.clip_rect(clip, None, Some(false));
    canvas.draw_rect(clip, &clear_paint(scene));
    render_scene_layers(canvas, scene, text_cache);
    canvas.restore();
}

fn clear_paint(scene: &Scene) -> sk::Paint {
    let mut paint = sk::Paint::default();
    paint.set_style(skia_safe::paint::Style::Fill);
    paint.set_anti_alias(true);
    let clear = scene
        .clear_color
        .or_else(|| scene.commands.iter().find_map(|cmd| match cmd {
            DrawCommand::Clear(color) => Some(*color),
            _ => None,
        }))
        .unwrap_or(Color::TRANSPARENT);
    paint.set_color(sk_color(clear));
    paint
}

fn draw_block(canvas: &sk::Canvas, block: &SceneBlock, text_cache: &mut SkiaTextCache) {
    let needs_save = !block.transform.is_identity() || block.clip.is_some();
    if needs_save {
        canvas.save();
    }
    if !block.transform.is_identity() {
        apply_transform(canvas, block.transform);
    }
    if let Some(clip) = block.clip {
        apply_clip(canvas, clip);
    }
    for cmd in &block.commands {
        draw_command(canvas, cmd, text_cache);
    }
    if needs_save {
        canvas.restore();
    }
}

fn apply_transform(canvas: &sk::Canvas, transform: Transform2D) {
    let matrix = sk::Matrix::new_all(
        transform.m11,
        transform.m21,
        transform.tx,
        transform.m12,
        transform.m22,
        transform.ty,
        0.0,
        0.0,
        1.0,
    );
    canvas.concat(&matrix);
}

fn render_scene_layers(canvas: &sk::Canvas, scene: &Scene, text_cache: &mut SkiaTextCache) {
    let layers_by_id: HashMap<u64, &SceneLayer> =
        scene.layers.iter().map(|layer| (layer.layer_id, layer)).collect();
    let mut child_layers_by_parent: HashMap<u64, Vec<&SceneLayer>> = HashMap::new();
    let mut blocks_by_layer: HashMap<u64, Vec<&SceneBlock>> = HashMap::new();
    for layer in &scene.layers {
        if let Some(parent_id) = layer.parent_layer_id {
            child_layers_by_parent.entry(parent_id).or_default().push(layer);
        }
    }
    for block in &scene.blocks {
        blocks_by_layer.entry(block.layer_id).or_default().push(block);
    }
    render_layer(
        canvas,
        scene,
        Scene::ROOT_LAYER_ID,
        &layers_by_id,
        &child_layers_by_parent,
        &blocks_by_layer,
        text_cache,
    );
}

fn render_layer(
    canvas: &sk::Canvas,
    scene: &Scene,
    layer_id: u64,
    layers_by_id: &HashMap<u64, &SceneLayer>,
    child_layers_by_parent: &HashMap<u64, Vec<&SceneLayer>>,
    blocks_by_layer: &HashMap<u64, Vec<&SceneBlock>>,
    text_cache: &mut SkiaTextCache,
) {
    let Some(layer) = layers_by_id.get(&layer_id).copied() else {
        return;
    };
    let initial_save_count = canvas.save_count();
    let mut saved = false;
    if layer.layer_id != Scene::ROOT_LAYER_ID {
        canvas.save();
        saved = true;
        if !layer.transform.is_identity() {
            apply_transform(canvas, layer.transform);
        }
        if let Some(clip) = layer.clip {
            apply_clip(canvas, clip);
        }
        if needs_save_layer(layer) {
            let bounds = layer_effect_bounds(layer);
            let paint = layer_paint(layer);
            let layer_rec = sk::canvas::SaveLayerRec::default()
                .bounds(&bounds)
                .paint(&paint);
            canvas.save_layer(&layer_rec);
        }
    }

    let mut items = Vec::new();
    if let Some(blocks) = blocks_by_layer.get(&layer_id) {
        for block in blocks {
            items.push((block.order, LayerItem::Block(*block)));
        }
    }
    if let Some(children) = child_layers_by_parent.get(&layer_id) {
        for child in children {
            items.push((child.order, LayerItem::Layer(child.layer_id)));
        }
    }
    items.sort_by_key(|(order, _)| *order);
    for (_, item) in items {
        match item {
            LayerItem::Block(block) => draw_block(canvas, block, text_cache),
            LayerItem::Layer(child_layer_id) => render_layer(
                canvas,
                scene,
                child_layer_id,
                layers_by_id,
                child_layers_by_parent,
                blocks_by_layer,
                text_cache,
            ),
        }
    }
    if saved {
        canvas.restore_to_count(initial_save_count);
    }
}

fn needs_save_layer(layer: &SceneLayer) -> bool {
    layer.offscreen
        || layer.opacity < 1.0
        || layer.blend_mode != SceneBlendMode::Normal
        || !layer.effects.is_empty()
}

fn layer_paint(layer: &SceneLayer) -> sk::Paint {
    let mut paint = sk::Paint::default();
    paint.set_anti_alias(true);
    paint.set_alpha_f(layer.opacity.clamp(0.0, 1.0));
    paint.set_blend_mode(sk_blend_mode(layer.blend_mode));
    if let Some(filter) = layer_image_filter(layer) {
        paint.set_image_filter(filter);
    }
    paint
}

fn sk_blend_mode(mode: SceneBlendMode) -> sk::BlendMode {
    match mode {
        SceneBlendMode::Normal => sk::BlendMode::SrcOver,
        SceneBlendMode::Multiply => sk::BlendMode::Multiply,
        SceneBlendMode::Screen => sk::BlendMode::Screen,
    }
}

fn layer_image_filter(layer: &SceneLayer) -> Option<sk::ImageFilter> {
    let mut current = None;
    for effect in &layer.effects {
        current = match effect {
            SceneEffect::Blur { sigma } => {
                sk::image_filters::blur((*sigma, *sigma), None, current, None)
            }
            SceneEffect::DropShadow {
                dx,
                dy,
                blur,
                color,
            } => sk::image_filters::drop_shadow(
                (*dx, *dy),
                (*blur, *blur),
                sk::Color4f::from(sk_color(*color)),
                None,
                current,
                None,
            ),
        };
    }
    current
}

fn layer_effect_bounds(layer: &SceneLayer) -> sk::Rect {
    let local_bounds = effect_bounds_for_scene_effects(layer.local_bounds, &layer.effects);
    sk::Rect::from_xywh(
        local_bounds.origin.x,
        local_bounds.origin.y,
        local_bounds.size.width,
        local_bounds.size.height,
    )
}

fn effect_bounds_for_scene_effects(bounds: Rect, effects: &[SceneEffect]) -> Rect {
    let mut visual_bounds = bounds;
    for effect in effects {
        match effect {
            SceneEffect::Blur { sigma } => {
                visual_bounds = expand_rect(visual_bounds, sigma * 3.0);
            }
            SceneEffect::DropShadow { dx, dy, blur, .. } => {
                let shadow_bounds = expand_rect(
                    Rect::new(
                        visual_bounds.origin.x + dx,
                        visual_bounds.origin.y + dy,
                        visual_bounds.size.width,
                        visual_bounds.size.height,
                    ),
                    blur * 3.0,
                );
                visual_bounds = visual_bounds.union(&shadow_bounds);
            }
        }
    }
    visual_bounds
}

fn expand_rect(rect: Rect, amount: f32) -> Rect {
    Rect::new(
        rect.origin.x - amount,
        rect.origin.y - amount,
        rect.size.width + amount * 2.0,
        rect.size.height + amount * 2.0,
    )
}

enum LayerItem<'a> {
    Block(&'a SceneBlock),
    Layer(u64),
}

fn apply_clip(canvas: &sk::Canvas, clip: SceneClip) {
    match clip {
        SceneClip::Rect(rect) => {
            canvas.clip_rect(
                sk::Rect::from_xywh(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height),
                None,
                Some(true),
            );
        }
        SceneClip::RoundedRect { rect, radius } => {
            let rrect = sk::RRect::new_rect_xy(
                sk::Rect::from_xywh(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height),
                radius,
                radius,
            );
            canvas.clip_rrect(rrect, None, Some(true));
        }
    }
}

fn draw_command(canvas: &sk::Canvas, cmd: &DrawCommand, text_cache: &mut SkiaTextCache) {
    match cmd {
        DrawCommand::Clear(color) => {
            canvas.clear(sk_color(*color));
        }
        DrawCommand::Fill { shape, brush } => {
            let mut paint = sk::Paint::default();
            paint.set_style(skia_safe::paint::Style::Fill);
            paint.set_anti_alias(true);
            let zeno_graphics::Brush::Solid(c) = brush;
            paint.set_color(sk_color(*c));
            draw_shape(canvas, shape, &paint);
        }
        DrawCommand::Stroke { shape, stroke } => {
            let mut paint = sk::Paint::default();
            paint.set_style(skia_safe::paint::Style::Stroke);
            paint.set_anti_alias(true);
            paint.set_stroke_width(stroke.width);
            paint.set_color(sk_color(stroke.color));
            draw_shape(canvas, shape, &paint);
        }
        DrawCommand::Text { position, layout, color } => {
            let mut paint = sk::Paint::default();
            paint.set_anti_alias(true);
            paint.set_color(sk_color(*color));
            let mut font = text_cache.resolve_font(
                cmd.resource_key(),
                &layout.paragraph.font.family,
                layout.paragraph.font_size.max(12.0),
            );
            font.set_edging(sk::font::Edging::AntiAlias);
            if layout.glyphs.iter().all(|glyph| glyph.glyph_id != 0) {
                let glyph_ids: Vec<u16> = layout.glyphs.iter().map(|glyph| glyph.glyph_id).collect();
                let positions: Vec<sk::Point> = layout
                    .glyphs
                    .iter()
                    .map(|glyph| sk::Point::new(glyph.x, glyph.baseline_y))
                    .collect();
                canvas.draw_glyphs_at(
                    &glyph_ids,
                    positions.as_slice(),
                    sk::Point::new(position.x, position.y),
                    &font,
                    &paint,
                );
            } else {
                canvas.draw_str(layout.paragraph.text.as_str(), (position.x, position.y), &font, &paint);
            }
        }
    }
}

pub fn sk_color(color: Color) -> sk::Color {
    sk::Color::from_argb(color.alpha, color.red, color.green, color.blue)
}

fn draw_shape(canvas: &sk::Canvas, shape: &Shape, paint: &sk::Paint) {
    match shape {
        Shape::Rect(rect) => {
            let rect = sk::Rect::from_xywh(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height);
            canvas.draw_rect(rect, paint);
        }
        Shape::RoundedRect { rect, radius } => {
            let rounded = sk::RRect::new_rect_xy(
                sk::Rect::from_xywh(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height),
                *radius,
                *radius,
            );
            canvas.draw_rrect(rounded, paint);
        }
        Shape::Circle { center, radius } => {
            canvas.draw_circle((center.x, center.y), *radius, paint);
        }
    }
}

impl SkiaTextCache {
    #[must_use]
    pub fn stats(&self) -> SkiaTextCacheStats {
        SkiaTextCacheStats {
            typeface_hits: self.stats.typeface_hits,
            font_hits: self.stats.font_hits,
            cached_typefaces: self.typefaces.len(),
            cached_fonts: self.fonts.len(),
        }
    }

    fn resolve_font(
        &mut self,
        resource_key: Option<SceneResourceKey>,
        requested_family: &str,
        font_size: f32,
    ) -> sk::Font {
        if let Some(resource_key) = resource_key {
            if let Some(font) = self.fonts.get(&resource_key) {
                self.stats.font_hits += 1;
                return font.clone();
            }
            let font = build_font(
                self.resolve_typeface(Some(resource_key), requested_family),
                font_size,
            );
            self.fonts.insert(resource_key, font.clone());
            return font;
        }
        build_font(self.resolve_typeface(None, requested_family), font_size)
    }

    fn resolve_typeface(
        &mut self,
        resource_key: Option<SceneResourceKey>,
        requested_family: &str,
    ) -> Option<sk::Typeface> {
        if let Some(resource_key) = resource_key {
            if let Some(typeface) = self.typefaces.get(&resource_key) {
                self.stats.typeface_hits += 1;
                return typeface.clone();
            }
            let resolved = resolve_typeface_uncached(requested_family);
            self.typefaces.insert(resource_key, resolved.clone());
            return resolved;
        }
        resolve_typeface_uncached(requested_family)
    }
}

fn build_font(typeface: Option<sk::Typeface>, font_size: f32) -> sk::Font {
    match typeface {
        Some(typeface) => sk::Font::from_typeface(typeface, font_size),
        None => {
            let mut font = sk::Font::default();
            font.set_size(font_size);
            font
        }
    }
}

fn resolve_typeface_uncached(requested_family: &str) -> Option<sk::Typeface> {
    let font_mgr = sk::FontMgr::default();
    let mut families = vec![requested_family, "PingFang SC", "Helvetica Neue", "Arial", "Noto Sans"];
    families.retain(|family| !family.is_empty() && *family != "System");

    for family in families {
        if let Some(typeface) = font_mgr.match_family_style(family, sk::FontStyle::normal()) {
            return Some(typeface);
        }
    }

    None
}

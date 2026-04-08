use std::collections::HashMap;

use skia_safe as sk;
use zeno_core::Color;
use zeno_graphics::{DrawCommand, Scene, SceneResourceKey, Shape};

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
    for cmd in &scene.commands {
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

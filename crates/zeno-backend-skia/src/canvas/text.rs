use std::collections::HashMap;

use skia_safe as sk;
use zeno_core::{Color, Point};
use zeno_scene::SceneResourceKey;
use zeno_text::{FontDescriptor, preferred_font_families};

use crate::canvas::mapping::sk_color;

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

    pub(crate) fn resolve_font(
        &mut self,
        resource_key: Option<SceneResourceKey>,
        descriptor: &FontDescriptor,
        font_size: f32,
    ) -> sk::Font {
        if let Some(resource_key) = resource_key {
            if let Some(font) = self.fonts.get(&resource_key) {
                self.stats.font_hits += 1;
                return font.clone();
            }

            // 文本资源键稳定时直接复用字体对象，避免一帧内重复走系统字体解析。
            let font = build_font(
                self.resolve_typeface(Some(resource_key), descriptor),
                font_size,
            );
            self.fonts.insert(resource_key, font.clone());
            return font;
        }
        build_font(self.resolve_typeface(None, descriptor), font_size)
    }

    fn resolve_typeface(
        &mut self,
        resource_key: Option<SceneResourceKey>,
        descriptor: &FontDescriptor,
    ) -> Option<sk::Typeface> {
        if let Some(resource_key) = resource_key {
            if let Some(typeface) = self.typefaces.get(&resource_key) {
                self.stats.typeface_hits += 1;
                return typeface.clone();
            }
            let resolved = resolve_typeface_uncached(descriptor);
            self.typefaces.insert(resource_key, resolved.clone());
            return resolved;
        }
        resolve_typeface_uncached(descriptor)
    }
}

pub(crate) fn draw_text_layout(
    canvas: &sk::Canvas,
    position: Point,
    layout: &zeno_text::TextLayout,
    color: Color,
    text_cache: &mut SkiaTextCache,
) {
    let mut paint = sk::Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(sk_color(color));
    let mut font = text_cache.resolve_font(
        Some(SceneResourceKey(layout.cache_key().stable_hash())),
        &layout.paragraph.font,
        layout.paragraph.font_size.max(12.0),
    );
    font.set_edging(sk::font::Edging::AntiAlias);
    let mut glyph_run = Vec::new();
    for glyph in &layout.glyphs {
        if glyph.glyph_id != 0 {
            glyph_run.push(glyph);
            continue;
        }
        flush_glyph_run(canvas, &glyph_run, position, &font, &paint);
        glyph_run.clear();
        canvas.draw_str(
            glyph.glyph.to_string(),
            (position.x + glyph.x, position.y + glyph.baseline_y),
            &font,
            &paint,
        );
    }
    flush_glyph_run(canvas, &glyph_run, position, &font, &paint);
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

fn resolve_typeface_uncached(descriptor: &FontDescriptor) -> Option<sk::Typeface> {
    let font_mgr = sk::FontMgr::default();
    for family in preferred_font_families(&descriptor.family) {
        if let Some(typeface) = font_mgr.match_family_style(family, font_style(descriptor)) {
            return Some(typeface);
        }
    }

    None
}

fn font_style(descriptor: &FontDescriptor) -> sk::FontStyle {
    sk::FontStyle::new(
        i32::from(descriptor.weight.0).into(),
        sk::font_style::Width::NORMAL,
        if descriptor.italic {
            sk::font_style::Slant::Italic
        } else {
            sk::font_style::Slant::Upright
        },
    )
}

fn flush_glyph_run(
    canvas: &sk::Canvas,
    glyph_run: &[&zeno_text::ShapedGlyph],
    position: Point,
    font: &sk::Font,
    paint: &sk::Paint,
) {
    if glyph_run.is_empty() {
        return;
    }
    let glyph_ids: Vec<u16> = glyph_run.iter().map(|glyph| glyph.glyph_id).collect();
    let positions: Vec<sk::Point> = glyph_run
        .iter()
        .map(|glyph| sk::Point::new(glyph.x, glyph.baseline_y))
        .collect();
    canvas.draw_glyphs_at(
        &glyph_ids,
        positions.as_slice(),
        sk::Point::new(position.x, position.y),
        font,
        paint,
    );
}

use std::collections::HashMap;

use skia_safe as sk;
use zeno_graphics::SceneResourceKey;

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
        requested_family: &str,
        font_size: f32,
    ) -> sk::Font {
        if let Some(resource_key) = resource_key {
            if let Some(font) = self.fonts.get(&resource_key) {
                self.stats.font_hits += 1;
                return font.clone();
            }

            // 文本资源键稳定时直接复用字体对象，避免一帧内重复走系统字体解析。
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
    let mut families = vec![
        requested_family,
        "PingFang SC",
        "Helvetica Neue",
        "Arial",
        "Noto Sans",
    ];
    families.retain(|family| !family.is_empty() && *family != "System");

    // 这里显式保留中文和通用西文字体回退，保证默认 macOS 桌面场景下的文本可读性。
    for family in families {
        if let Some(typeface) = font_mgr.match_family_style(family, sk::FontStyle::normal()) {
            return Some(typeface);
        }
    }

    None
}

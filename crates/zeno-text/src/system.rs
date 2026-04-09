use std::sync::OnceLock;

use crate::{
    TextCapabilities, TextLayout, TextParagraph,
    cache::{ParagraphTextCache, TextCache, TextCacheStats},
    shaper::{FallbackTextShaper, TextShaper},
};

pub trait TextSystem: Send + Sync {
    fn name(&self) -> &'static str;

    fn capabilities(&self) -> TextCapabilities;

    fn layout(&self, paragraph: TextParagraph) -> TextLayout;

    fn cache_stats(&self) -> Option<TextCacheStats> {
        None
    }

    fn reset_caches(&self) {}
}

#[derive(Debug)]
pub struct CachedTextSystem<S, C> {
    name: &'static str,
    shaper: S,
    cache: C,
    capabilities: TextCapabilities,
}

impl<S, C> CachedTextSystem<S, C> {
    #[must_use]
    pub const fn new(
        name: &'static str,
        shaper: S,
        cache: C,
        capabilities: TextCapabilities,
    ) -> Self {
        Self {
            name,
            shaper,
            cache,
            capabilities,
        }
    }
}

impl<S, C> TextSystem for CachedTextSystem<S, C>
where
    S: TextShaper,
    C: TextCache,
{
    fn name(&self) -> &'static str {
        self.name
    }

    fn capabilities(&self) -> TextCapabilities {
        self.capabilities.clone()
    }

    fn layout(&self, paragraph: TextParagraph) -> TextLayout {
        let key = paragraph.cache_key();
        if let Some(layout) = self.cache.get(key) {
            return layout;
        }
        let layout = self.shaper.shape(paragraph);
        self.cache.insert(key, layout.clone());
        layout
    }

    fn cache_stats(&self) -> Option<TextCacheStats> {
        Some(self.cache.stats())
    }

    fn reset_caches(&self) {
        self.cache.reset();
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FallbackTextSystem;

fn fallback_cache() -> &'static ParagraphTextCache {
    static CACHE: OnceLock<ParagraphTextCache> = OnceLock::new();
    CACHE.get_or_init(ParagraphTextCache::default)
}

impl FallbackTextSystem {
    #[must_use]
    pub fn cache_stats() -> TextCacheStats {
        fallback_cache().stats()
    }

    pub fn reset_cache() {
        fallback_cache().reset();
    }
}

impl TextSystem for FallbackTextSystem {
    fn name(&self) -> &'static str {
        "fallback-text"
    }

    fn capabilities(&self) -> TextCapabilities {
        TextCapabilities {
            paragraph_cache: true,
            ..TextCapabilities::minimal()
        }
    }

    fn layout(&self, paragraph: TextParagraph) -> TextLayout {
        let key = paragraph.cache_key();
        if let Some(layout) = fallback_cache().get(key) {
            return layout;
        }
        let layout = FallbackTextShaper.shape(paragraph);
        fallback_cache().insert(key, layout.clone());
        layout
    }

    fn cache_stats(&self) -> Option<TextCacheStats> {
        Some(Self::cache_stats())
    }

    fn reset_caches(&self) {
        Self::reset_cache()
    }
}

#[cfg(test)]
mod tests {
    use super::{FallbackTextSystem, TextSystem};
    use crate::TextParagraph;

    #[test]
    fn fallback_text_system_records_cache_hits() {
        FallbackTextSystem::reset_cache();
        let paragraph = TextParagraph::new("Hello cache", 120.0);

        let _ = FallbackTextSystem.layout(paragraph.clone());
        let _ = FallbackTextSystem.layout(paragraph);

        let stats = FallbackTextSystem::cache_stats();
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }
}

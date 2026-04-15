use std::collections::HashMap;

use metal::{Device, Texture};
use zeno_core::Rect;
use zeno_scene::{ImageCacheKey, StackingContextId};

use super::super::draw::make_rgba_texture;

#[derive(Default)]
pub(crate) struct OffscreenContextCache {
    entries: HashMap<StackingContextId, CachedOffscreenContext>,
    hits: usize,
    misses: usize,
}

#[derive(Clone)]
pub(crate) struct CachedOffscreenContext {
    pub texture: Texture,
    pub texture_scene_bounds: Rect,
    pub texture_width: u64,
    pub texture_height: u64,
    pub covered_rect: Rect,
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct OffscreenContextCacheStats {
    pub entry_count: usize,
    pub hits: usize,
    pub misses: usize,
}

impl OffscreenContextCache {
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub(crate) fn get(&mut self, context_id: StackingContextId) -> Option<CachedOffscreenContext> {
        match self.entries.get(&context_id).cloned() {
            Some(entry) => {
                self.hits += 1;
                Some(entry)
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    pub(crate) fn insert(&mut self, context_id: StackingContextId, entry: CachedOffscreenContext) {
        self.entries.insert(context_id, entry);
    }

    pub(crate) fn stats(&self) -> OffscreenContextCacheStats {
        OffscreenContextCacheStats {
            entry_count: self.entries.len(),
            hits: self.hits,
            misses: self.misses,
        }
    }
}

pub(crate) struct ImageTextureCache {
    entries: HashMap<ImageCacheKey, CachedImageTexture>,
    total_bytes: usize,
    budget_bytes: usize,
    use_clock: u64,
}

struct CachedImageTexture {
    texture: Texture,
    width: u32,
    height: u32,
    byte_size: usize,
    last_used: u64,
}

impl ImageTextureCache {
    pub(crate) fn new(budget_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            total_bytes: 0,
            budget_bytes,
            use_clock: 0,
        }
    }

    pub(crate) fn texture_for_image(
        &mut self,
        device: &Device,
        cache_key: ImageCacheKey,
        rgba8: &[u8],
        width: u32,
        height: u32,
    ) -> Texture {
        self.use_clock = self.use_clock.saturating_add(1);
        let current_use = self.use_clock;
        if let Some(entry) = self.entries.get_mut(&cache_key) {
            if entry.width == width && entry.height == height {
                entry.last_used = current_use;
                return entry.texture.clone();
            }
            self.total_bytes = self.total_bytes.saturating_sub(entry.byte_size);
        }

        let texture = make_rgba_texture(device, rgba8, width, height);
        let byte_size = width as usize * height as usize * 4;
        self.entries.insert(
            cache_key,
            CachedImageTexture {
                texture: texture.clone(),
                width,
                height,
                byte_size,
                last_used: current_use,
            },
        );
        self.total_bytes += byte_size;
        self.evict_to_budget(cache_key);
        texture
    }

    fn evict_to_budget(&mut self, keep_key: ImageCacheKey) {
        while self.total_bytes > self.budget_bytes && self.entries.len() > 1 {
            let Some(evict_key) = self
                .entries
                .iter()
                .filter(|(key, _)| **key != keep_key)
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| *key)
            else {
                break;
            };
            if let Some(entry) = self.entries.remove(&evict_key) {
                self.total_bytes = self.total_bytes.saturating_sub(entry.byte_size);
            }
        }
    }
}

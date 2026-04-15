use std::collections::HashMap;

use skia_safe as sk;
use zeno_scene::ImageCacheKey;

#[derive(Default)]
pub struct SkiaImageCache {
    images: HashMap<ImageCacheKey, CachedImage>,
    stats: SkiaImageCacheStats,
}

#[derive(Clone)]
struct CachedImage {
    image: sk::Image,
    width: u32,
    height: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SkiaImageCacheStats {
    pub image_hits: usize,
    pub cached_images: usize,
}

impl SkiaImageCache {
    #[must_use]
    pub fn stats(&self) -> SkiaImageCacheStats {
        SkiaImageCacheStats {
            image_hits: self.stats.image_hits,
            cached_images: self.images.len(),
        }
    }

    pub(crate) fn resolve_rgba8(
        &mut self,
        cache_key: ImageCacheKey,
        width: u32,
        height: u32,
        rgba8: &[u8],
    ) -> Option<sk::Image> {
        if let Some(cached) = self.images.get(&cache_key) {
            if cached.width == width && cached.height == height {
                self.stats.image_hits += 1;
                return Some(cached.image.clone());
            }
        }

        let info = sk::ImageInfo::new(
            (width as i32, height as i32),
            sk::ColorType::RGBA8888,
            sk::AlphaType::Premul,
            None,
        );
        let image =
            sk::images::raster_from_data(&info, sk::Data::new_copy(rgba8), (width * 4) as usize)?;
        self.images.insert(
            cache_key,
            CachedImage {
                image: image.clone(),
                width,
                height,
            },
        );
        Some(image)
    }
}

#[cfg(test)]
mod tests {
    use super::SkiaImageCache;
    use zeno_scene::ImageCacheKey;

    #[test]
    fn reuses_cached_images_for_same_key() {
        let mut cache = SkiaImageCache::default();
        let pixels = [
            255_u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];

        let first = cache
            .resolve_rgba8(ImageCacheKey(1), 2, 2, &pixels)
            .expect("image should decode");
        let second = cache
            .resolve_rgba8(ImageCacheKey(1), 2, 2, &pixels)
            .expect("image should hit cache");

        assert_eq!(first.dimensions(), second.dimensions());
        assert_eq!(cache.stats().cached_images, 1);
        assert_eq!(cache.stats().image_hits, 1);
    }
}

use std::collections::HashMap;
use std::sync::Arc;

use crate::frontend::{FrontendObjectKind, FrontendObjectTable};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageResourceKey(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub enum ImageSource {
    Rgba8 {
        width: u32,
        height: u32,
        rgba8: Arc<[u8]>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageResource {
    pub key: ImageResourceKey,
    pub width: u32,
    pub height: u32,
    pub rgba8: Arc<[u8]>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImageResourceTable {
    entries: HashMap<ImageResourceKey, ImageResource>,
}

impl ImageSource {
    #[must_use]
    pub fn rgba8(width: u32, height: u32, rgba8: impl Into<Arc<[u8]>>) -> Self {
        let rgba8 = rgba8.into();
        debug_assert_eq!(
            rgba8.len(),
            (width as usize) * (height as usize) * 4,
            "ImageSource::rgba8 expects RGBA8 pixel storage"
        );
        Self::Rgba8 {
            width,
            height,
            rgba8,
        }
    }

    #[must_use]
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            Self::Rgba8 { width, height, .. } => (*width, *height),
        }
    }

    #[must_use]
    pub fn resource_key(&self) -> ImageResourceKey {
        let mut hash = 0xcbf29ce484222325u64;
        match self {
            Self::Rgba8 {
                width,
                height,
                rgba8,
            } => {
                hash = stable_hash_u32(hash, *width);
                hash = stable_hash_u32(hash, *height);
                for byte in rgba8.iter() {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(0x100000001b3);
                }
            }
        }
        ImageResourceKey(hash)
    }

    #[must_use]
    pub fn to_resource(&self) -> ImageResource {
        match self {
            Self::Rgba8 {
                width,
                height,
                rgba8,
            } => ImageResource {
                key: self.resource_key(),
                width: *width,
                height: *height,
                rgba8: rgba8.clone(),
            },
        }
    }
}

impl ImageResourceTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_source(&mut self, source: &ImageSource) -> ImageResourceKey {
        let resource = source.to_resource();
        let key = resource.key;
        self.entries.entry(key).or_insert(resource);
        key
    }

    #[must_use]
    pub fn resolve(&self, key: ImageResourceKey) -> Option<&ImageResource> {
        self.entries.get(&key)
    }

    #[cfg(test)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub(crate) fn from_frontend(objects: &FrontendObjectTable) -> Self {
        let mut table = Self::new();
        for object in &objects.objects {
            if let FrontendObjectKind::Image(image) = &object.kind {
                table.insert_source(&image.source);
            }
        }
        table
    }
}

fn stable_hash_u32(mut hash: u64, value: u32) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{ImageResourceTable, ImageSource};

    #[test]
    fn resource_table_deduplicates_identical_rgba_sources() {
        let source = ImageSource::rgba8(1, 1, vec![1, 2, 3, 255]);
        let mut table = ImageResourceTable::new();

        let a = table.insert_source(&source);
        let b = table.insert_source(&source);

        assert_eq!(a, b);
        assert_eq!(table.len(), 1);
    }
}

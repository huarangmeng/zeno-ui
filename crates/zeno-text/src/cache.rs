use std::collections::HashMap;
use std::sync::Mutex;

use crate::{TextLayout, TextParagraphKey};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextCacheStats {
    pub entries: usize,
    pub hits: usize,
    pub misses: usize,
}

pub trait TextCache: Send + Sync {
    fn get(&self, key: TextParagraphKey) -> Option<TextLayout>;

    fn insert(&self, key: TextParagraphKey, layout: TextLayout);

    fn stats(&self) -> TextCacheStats;

    fn reset(&self);
}

#[derive(Debug, Default)]
pub struct ParagraphTextCache {
    inner: Mutex<ParagraphTextCacheState>,
}

#[derive(Debug, Default)]
struct ParagraphTextCacheState {
    layouts: HashMap<TextParagraphKey, TextLayout>,
    hits: usize,
    misses: usize,
}

impl TextCache for ParagraphTextCache {
    fn get(&self, key: TextParagraphKey) -> Option<TextLayout> {
        let mut inner = self.inner.lock().expect("paragraph text cache");
        let layout = inner.layouts.get(&key).cloned();
        if layout.is_some() {
            inner.hits += 1;
        } else {
            inner.misses += 1;
        }
        layout
    }

    fn insert(&self, key: TextParagraphKey, layout: TextLayout) {
        let mut inner = self.inner.lock().expect("paragraph text cache");
        inner.layouts.insert(key, layout);
    }

    fn stats(&self) -> TextCacheStats {
        let inner = self.inner.lock().expect("paragraph text cache");
        TextCacheStats {
            entries: inner.layouts.len(),
            hits: inner.hits,
            misses: inner.misses,
        }
    }

    fn reset(&self) {
        let mut inner = self.inner.lock().expect("paragraph text cache");
        inner.layouts.clear();
        inner.hits = 0;
        inner.misses = 0;
    }
}

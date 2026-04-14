use zeno_core::Rect;

#[derive(Debug, Clone, PartialEq)]
pub enum DamageRegion {
    Empty,
    Rects(Vec<Rect>),
    Full,
}

impl DamageRegion {
    #[must_use]
    pub fn single_rect_or_full(rect: Option<Rect>) -> Self {
        rect.map_or(Self::Full, |rect| Self::Rects(vec![rect]))
    }

    #[must_use]
    pub fn from_rects(rects: impl IntoIterator<Item = Rect>) -> Self {
        let rects: Vec<_> = rects.into_iter().collect();
        if rects.is_empty() {
            Self::Empty
        } else {
            Self::Rects(rects)
        }
    }

    #[must_use]
    pub fn bounds(&self) -> Option<Rect> {
        match self {
            Self::Empty => None,
            Self::Full => None,
            Self::Rects(rects) => rects.iter().copied().reduce(|current, rect| current.union(&rect)),
        }
    }

    #[must_use]
    pub fn rect_count(&self) -> usize {
        match self {
            Self::Empty | Self::Full => 0,
            Self::Rects(rects) => rects.len(),
        }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    #[must_use]
    pub const fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct DamageTracker {
    full: bool,
    rects: Vec<Rect>,
}

impl DamageTracker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_full(&mut self) {
        self.full = true;
        self.rects.clear();
    }

    pub fn add_rect(&mut self, rect: Rect) {
        if self.full {
            return;
        }
        self.rects.push(rect);
    }

    pub fn add_optional_rect(&mut self, rect: Option<Rect>) {
        if let Some(rect) = rect {
            self.add_rect(rect);
        }
    }

    #[must_use]
    pub fn build(self) -> DamageRegion {
        if self.full {
            DamageRegion::Full
        } else {
            DamageRegion::from_rects(self.rects)
        }
    }
}

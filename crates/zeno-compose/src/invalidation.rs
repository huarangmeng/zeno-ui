#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyReason {
    Structure,
    Layout,
    Paint,
    Text,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DirtyFlags {
    pub layout: bool,
    pub paint: bool,
    pub text: bool,
}

impl DirtyFlags {
    #[must_use]
    pub const fn clean() -> Self {
        Self {
            layout: false,
            paint: false,
            text: false,
        }
    }

    pub fn mark(&mut self, reason: DirtyReason) {
        match reason {
            DirtyReason::Structure | DirtyReason::Layout => {
                self.layout = true;
                self.paint = true;
            }
            DirtyReason::Paint => {
                self.paint = true;
            }
            DirtyReason::Text => {
                self.text = true;
                self.layout = true;
                self.paint = true;
            }
        }
    }

    #[must_use]
    pub const fn requires_layout(self) -> bool {
        self.layout || self.text
    }

    #[must_use]
    pub const fn requires_paint_only(self) -> bool {
        self.paint && !self.requires_layout()
    }

    #[must_use]
    pub const fn is_clean(self) -> bool {
        !self.layout && !self.paint && !self.text
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FramePhases {
    pub needs_layout: bool,
    pub needs_paint: bool,
    pub needs_present: bool,
}

impl FramePhases {
    #[must_use]
    pub const fn any(self) -> bool {
        self.needs_layout || self.needs_paint || self.needs_present
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FrameScheduler {
    pending: FramePhases,
}

impl FrameScheduler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn invalidate_layout(&mut self) {
        self.pending.needs_layout = true;
        self.pending.needs_paint = true;
        self.pending.needs_present = true;
    }

    pub fn invalidate_paint(&mut self) {
        self.pending.needs_paint = true;
        self.pending.needs_present = true;
    }

    pub fn invalidate_present(&mut self) {
        self.pending.needs_present = true;
    }

    #[must_use]
    pub fn has_pending_frame(&self) -> bool {
        self.pending.any()
    }

    #[must_use]
    pub fn pending(&self) -> FramePhases {
        self.pending
    }

    pub fn finish_frame(&mut self) {
        self.pending = FramePhases::default();
    }
}

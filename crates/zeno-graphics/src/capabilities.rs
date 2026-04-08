use zeno_core::{Backend, BackendUnavailableReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderCapabilities {
    pub gpu_compositing: bool,
    pub text_shaping: bool,
    pub filters: bool,
    pub offscreen_rendering: bool,
}

impl RenderCapabilities {
    #[must_use]
    pub const fn minimal() -> Self {
        Self {
            gpu_compositing: true,
            text_shaping: true,
            filters: false,
            offscreen_rendering: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendProbe {
    pub kind: Backend,
    pub available: bool,
    pub reason: Option<BackendUnavailableReason>,
    pub capabilities: RenderCapabilities,
}

impl BackendProbe {
    #[must_use]
    pub fn available(kind: Backend, capabilities: RenderCapabilities) -> Self {
        Self {
            kind,
            available: true,
            reason: None,
            capabilities,
        }
    }

    #[must_use]
    pub fn unavailable(kind: Backend, reason: BackendUnavailableReason) -> Self {
        Self {
            kind,
            available: false,
            reason: Some(reason),
            capabilities: RenderCapabilities::minimal(),
        }
    }
}

use zeno_core::WindowConfig;

use crate::{BackendAttempt, ResolvedBackend};

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSession {
    pub window: WindowConfig,
    pub backend: ResolvedBackend,
    pub frame_stats: bool,
}

impl ResolvedSession {
    #[must_use]
    pub fn new(window: WindowConfig, backend: ResolvedBackend, frame_stats: bool) -> Self {
        Self {
            window,
            backend,
            frame_stats,
        }
    }

    #[must_use]
    pub fn attempts(&self) -> &[BackendAttempt] {
        &self.backend.attempts
    }
}

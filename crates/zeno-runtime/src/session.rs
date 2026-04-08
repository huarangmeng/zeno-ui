use zeno_core::{AppConfig, Platform, RendererConfig, WindowConfig, ZenoError};

use crate::{BackendAttempt, BackendResolver, ResolvedBackend};

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSession {
    pub platform: Platform,
    pub window: WindowConfig,
    pub backend: ResolvedBackend,
    pub frame_stats: bool,
}

impl ResolvedSession {
    #[must_use]
    pub fn new(
        platform: Platform,
        window: WindowConfig,
        backend: ResolvedBackend,
        frame_stats: bool,
    ) -> Self {
        Self {
            platform,
            window,
            backend,
            frame_stats,
        }
    }

    pub fn from_parts(
        platform: Platform,
        window: WindowConfig,
        renderer: &RendererConfig,
        frame_stats: bool,
    ) -> Result<Self, ZenoError> {
        let backend = BackendResolver::new().resolve_backend(platform, renderer)?;
        Ok(Self::new(platform, window, backend, frame_stats))
    }

    pub fn resolve(platform: Platform, app_config: &AppConfig) -> Result<Self, ZenoError> {
        Self::from_parts(
            platform,
            app_config.window.clone(),
            &app_config.renderer,
            app_config.debug.frame_stats,
        )
    }

    #[must_use]
    pub fn attempts(&self) -> &[BackendAttempt] {
        &self.backend.attempts
    }
}

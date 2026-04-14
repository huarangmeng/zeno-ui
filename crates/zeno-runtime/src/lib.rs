pub mod animation;
mod app;
mod host;
pub mod input;
pub mod lifecycle;
mod scheduler;

pub use app::{App, AppFrame, AppView, PointerState};
pub use host::{AppHost, UiFrame, UiRuntime, run_app, run_app_with_text_system};
pub use scheduler::{FramePhases, FrameScheduler};

#[cfg(test)]
mod tests {
    use super::FrameScheduler;
    use zeno_core::{
        Backend, BackendPreference, Platform, RendererConfig, WindowConfig, ZenoError,
    };
    use zeno_platform::{BackendResolver, ResolvedSession};

    #[test]
    fn falls_back_to_skia_when_impeller_is_not_implemented_yet() {
        let resolver = BackendResolver::new();
        let resolved = resolver
            .resolve_backend(Platform::Android, &RendererConfig::default())
            .expect("android should fall back to skia until impeller is implemented");

        assert_eq!(resolved.backend_kind, Backend::Skia);
        assert_eq!(resolved.attempts.len(), 2);
        assert!(resolved.attempts[0].reason.is_some());
    }

    #[test]
    fn falls_back_to_skia_when_impeller_is_unavailable() {
        let resolver = BackendResolver::new();
        let resolved = resolver
            .resolve_backend(Platform::Linux, &RendererConfig::default())
            .expect("linux should fall back to skia");

        assert_eq!(resolved.backend_kind, Backend::Skia);
        assert_eq!(resolved.attempts.len(), 2);
        assert!(resolved.attempts[0].reason.is_some());
    }

    #[test]
    fn returns_error_when_forced_backend_is_unavailable() {
        let resolver = BackendResolver::new();
        let config = RendererConfig {
            preference: BackendPreference::Force(Backend::Impeller),
            allow_fallback: false,
        };

        let error = match resolver.resolve_backend(Platform::Windows, &config) {
            Ok(_) => panic!("forced impeller should fail on windows"),
            Err(error) => error,
        };

        match error {
            ZenoError::BackendUnavailable { backend, .. } => {
                assert_eq!(backend, Backend::Impeller);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn honors_explicit_skia_override() {
        let resolver = BackendResolver::new();
        let config = RendererConfig {
            preference: BackendPreference::Force(Backend::Skia),
            allow_fallback: false,
        };
        let resolved = resolver
            .resolve_backend(Platform::Android, &config)
            .expect("skia should resolve everywhere");

        assert_eq!(resolved.backend_kind, Backend::Skia);
        assert_eq!(resolved.attempts.len(), 1);
    }

    #[test]
    fn resolved_session_captures_platform_and_attempts() {
        let session = ResolvedSession::from_parts(
            Platform::Linux,
            WindowConfig::default(),
            &RendererConfig::default(),
            true,
        )
        .expect("linux should produce a resolved session");

        assert_eq!(session.platform, Platform::Linux);
        assert_eq!(session.backend.backend_kind, Backend::Skia);
        assert_eq!(session.attempts().len(), 2);
        assert!(session.attempts()[0].reason.is_some());
        assert!(session.frame_stats);
    }

    #[test]
    fn resolve_backend_returns_attempts_without_constructing_renderer() {
        let resolver = BackendResolver::new();
        let resolved = resolver
            .resolve_backend(Platform::Linux, &RendererConfig::default())
            .expect("linux should resolve backend");

        assert_eq!(resolved.backend_kind, Backend::Skia);
        assert_eq!(resolved.attempts.len(), 2);
        assert!(resolved.attempts[0].reason.is_some());
    }

    #[test]
    fn scheduler_only_requests_frames_when_invalidated() {
        let mut scheduler = FrameScheduler::new();

        assert!(!scheduler.has_pending_frame());

        scheduler.invalidate_layout();
        assert!(scheduler.has_pending_frame());
        assert!(scheduler.pending().needs_layout);
        assert!(scheduler.pending().needs_paint);
        assert!(scheduler.pending().needs_present);

        scheduler.finish_frame();
        assert!(!scheduler.has_pending_frame());
    }
}

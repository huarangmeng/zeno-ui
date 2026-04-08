mod frame_scheduler;
mod resolver;
mod session;

pub use frame_scheduler::{FramePhases, FrameScheduler};
pub use resolver::{BackendAttempt, BackendResolver, ResolvedBackend, ResolvedRenderer};
pub use session::ResolvedSession;

#[cfg(test)]
mod tests {
    use super::{BackendResolver, FrameScheduler};
    use zeno_core::{Backend, BackendPreference, Platform, RendererConfig, ZenoError};

    #[test]
    fn falls_back_to_skia_when_impeller_is_not_implemented_yet() {
        let resolver = BackendResolver::new();
        let resolved = resolver
            .resolve(Platform::Android, &RendererConfig::default())
            .expect("android should fall back to skia until impeller is implemented");

        assert_eq!(resolved.backend_kind, Backend::Skia);
        assert_eq!(resolved.attempts.len(), 2);
        assert!(resolved.attempts[0].reason.is_some());
    }

    #[test]
    fn falls_back_to_skia_when_impeller_is_unavailable() {
        let resolver = BackendResolver::new();
        let resolved = resolver
            .resolve(Platform::Linux, &RendererConfig::default())
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

        let error = match resolver.resolve(Platform::Windows, &config) {
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
            .resolve(Platform::Android, &config)
            .expect("skia should resolve everywhere");

        assert_eq!(resolved.backend_kind, Backend::Skia);
        assert_eq!(resolved.attempts.len(), 1);
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

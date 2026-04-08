use zeno_backend_impeller::ImpellerBackend;
use zeno_backend_skia::SkiaBackend;
use zeno_core::{
    BackendKind, BackendPreference, BackendUnavailableReason, PlatformKind, RendererConfig,
    ZenoError,
};
use zeno_graphics::{GraphicsBackend, Renderer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendAttempt {
    pub backend: BackendKind,
    pub reason: Option<BackendUnavailableReason>,
}

pub struct ResolvedRenderer {
    pub backend_kind: BackendKind,
    pub renderer: Box<dyn Renderer>,
    pub attempts: Vec<BackendAttempt>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BackendResolver {
    impeller: ImpellerBackend,
    skia: SkiaBackend,
}

impl BackendResolver {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn resolve(
        &self,
        platform: PlatformKind,
        config: &RendererConfig,
    ) -> Result<ResolvedRenderer, ZenoError> {
        let order = self.order_for(&config.preference);
        let mut attempts = Vec::new();
        let mut failures = Vec::new();

        for backend in order {
            match self.try_backend(backend, platform) {
                Ok(renderer) => {
                    attempts.push(BackendAttempt {
                        backend,
                        reason: None,
                    });
                    return Ok(ResolvedRenderer {
                        backend_kind: backend,
                        renderer,
                        attempts,
                    });
                }
                Err(reason) => {
                    attempts.push(BackendAttempt {
                        backend,
                        reason: Some(reason.clone()),
                    });
                    failures.push((backend, reason));
                }
            }

            if !config.allow_fallback {
                break;
            }
        }

        if failures.len() == 1 {
            let (backend, reason) = failures.remove(0);
            Err(ZenoError::BackendUnavailable { backend, reason })
        } else {
            Err(ZenoError::NoBackendAvailable { attempts: failures })
        }
    }

    fn order_for(&self, preference: &BackendPreference) -> Vec<BackendKind> {
        match preference {
            BackendPreference::Auto | BackendPreference::PreferImpeller => {
                vec![BackendKind::Impeller, BackendKind::Skia]
            }
            BackendPreference::PreferSkia => vec![BackendKind::Skia, BackendKind::Impeller],
            BackendPreference::Force(kind) => vec![*kind],
        }
    }

    fn try_backend(
        &self,
        backend: BackendKind,
        platform: PlatformKind,
    ) -> Result<Box<dyn Renderer>, BackendUnavailableReason> {
        let backend_impl: &dyn GraphicsBackend = match backend {
            BackendKind::Impeller => &self.impeller,
            BackendKind::Skia => &self.skia,
        };
        let probe = backend_impl.probe(platform);
        if !probe.available {
            return Err(probe
                .reason
                .unwrap_or(BackendUnavailableReason::RuntimeProbeFailed(
                    "backend probe returned unavailable without reason".to_string(),
                )));
        }
        backend_impl.create_renderer().map_err(|error| match error {
            ZenoError::BackendUnavailable { reason, .. } => reason,
            other => BackendUnavailableReason::RuntimeProbeFailed(other.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::BackendResolver;
    use zeno_core::{BackendKind, BackendPreference, PlatformKind, RendererConfig, ZenoError};

    #[test]
    fn prefers_impeller_when_available() {
        let resolver = BackendResolver::new();
        let resolved = resolver
            .resolve(PlatformKind::Android, &RendererConfig::default())
            .expect("android should resolve to impeller");

        assert_eq!(resolved.backend_kind, BackendKind::Impeller);
    }

    #[test]
    fn falls_back_to_skia_when_impeller_is_unavailable() {
        let resolver = BackendResolver::new();
        let resolved = resolver
            .resolve(PlatformKind::Linux, &RendererConfig::default())
            .expect("linux should fall back to skia");

        assert_eq!(resolved.backend_kind, BackendKind::Skia);
        assert_eq!(resolved.attempts.len(), 2);
        assert!(resolved.attempts[0].reason.is_some());
    }

    #[test]
    fn returns_error_when_forced_backend_is_unavailable() {
        let resolver = BackendResolver::new();
        let config = RendererConfig {
            preference: BackendPreference::Force(BackendKind::Impeller),
            allow_fallback: false,
        };

        let error = match resolver.resolve(PlatformKind::Windows, &config) {
            Ok(_) => panic!("forced impeller should fail on windows"),
            Err(error) => error,
        };

        match error {
            ZenoError::BackendUnavailable { backend, .. } => {
                assert_eq!(backend, BackendKind::Impeller);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn honors_explicit_skia_override() {
        let resolver = BackendResolver::new();
        let config = RendererConfig {
            preference: BackendPreference::Force(BackendKind::Skia),
            allow_fallback: false,
        };
        let resolved = resolver
            .resolve(PlatformKind::Android, &config)
            .expect("skia should resolve everywhere");

        assert_eq!(resolved.backend_kind, BackendKind::Skia);
        assert_eq!(resolved.attempts.len(), 1);
    }
}

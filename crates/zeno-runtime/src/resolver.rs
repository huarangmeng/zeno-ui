use zeno_backend_impeller::ImpellerBackend;
use zeno_backend_skia::SkiaBackend;
use zeno_core::{
    Backend, BackendPreference, BackendUnavailableReason, Platform, RendererConfig,
    ZenoError,
};
use zeno_graphics::{GraphicsBackend, Renderer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendAttempt {
    pub backend: Backend,
    pub reason: Option<BackendUnavailableReason>,
}

pub struct ResolvedRenderer {
    pub backend_kind: Backend,
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
        platform: Platform,
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

    fn order_for(&self, preference: &BackendPreference) -> Vec<Backend> {
        match preference {
            BackendPreference::Auto | BackendPreference::PreferImpeller => {
                vec![Backend::Impeller, Backend::Skia]
            }
            BackendPreference::PreferSkia => vec![Backend::Skia, Backend::Impeller],
            BackendPreference::Force(kind) => vec![*kind],
        }
    }

    fn try_backend(
        &self,
        backend: Backend,
        platform: Platform,
    ) -> Result<Box<dyn Renderer>, BackendUnavailableReason> {
        let backend_impl: &dyn GraphicsBackend = match backend {
            Backend::Impeller => &self.impeller,
            Backend::Skia => &self.skia,
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

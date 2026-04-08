use zeno_backend_impeller::ImpellerBackend;
use zeno_backend_skia::SkiaBackend;
use zeno_core::{Backend, BackendPreference, BackendUnavailableReason, Platform, RendererConfig, ZenoError, ZenoErrorCode, zeno_runtime_log};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBackend {
    pub backend_kind: Backend,
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
                    zeno_runtime_log!(
                        info,
                        op = "resolve_renderer",
                        status = "success",
                        backend = ?backend,
                        platform = ?platform,
                        attempts = attempts.len(),
                        fallback_used = attempts.len() > 1,
                        "renderer resolved"
                    );
                    return Ok(ResolvedRenderer {
                        backend_kind: backend,
                        renderer,
                        attempts,
                    });
                }
                Err(reason) => {
                    let error = ZenoError::BackendUnavailable {
                        backend,
                        reason: reason.clone(),
                    };
                    attempts.push(BackendAttempt {
                        backend,
                        reason: Some(reason.clone()),
                    });
                    zeno_runtime_log!(
                        warn,
                        event = "backend_probe_failed",
                        error_code = %error.error_code(),
                        component = error.component(),
                        op = error.operation(),
                        error_kind = error.error_kind(),
                        message = %error.message(),
                        status = "degraded",
                        backend = ?backend,
                        platform = ?platform,
                        attempts = attempts.len(),
                        fallback_enabled = config.allow_fallback,
                        "backend probe failed"
                    );
                    failures.push((backend, reason));
                }
            }

            if !config.allow_fallback {
                break;
            }
        }

        if failures.len() == 1 {
            let (backend, reason) = failures.remove(0);
            let error = ZenoError::BackendUnavailable {
                backend,
                reason: reason.clone(),
            };
            zeno_runtime_log!(
                error,
                event = "backend_resolution_failed",
                error_code = %error.error_code(),
                component = error.component(),
                op = error.operation(),
                error_kind = error.error_kind(),
                message = %error.message(),
                status = "fail",
                backend = ?backend,
                platform = ?platform,
                attempts = attempts.len(),
                fallback_enabled = config.allow_fallback,
                "renderer resolution failed"
            );
            Err(error)
        } else {
            let error = ZenoError::NoBackendAvailable {
                attempts: failures.clone(),
            };
            zeno_runtime_log!(
                error,
                event = "backend_resolution_failed",
                error_code = %error.error_code(),
                component = error.component(),
                op = error.operation(),
                error_kind = error.error_kind(),
                message = %error.message(),
                status = "fail",
                platform = ?platform,
                attempts = attempts.len(),
                fallback_enabled = config.allow_fallback,
                failure_count = failures.len(),
                "renderer resolution failed"
            );
            Err(error)
        }
    }

    pub fn resolve_backend(
        &self,
        platform: Platform,
        config: &RendererConfig,
    ) -> Result<ResolvedBackend, ZenoError> {
        let order = self.order_for(&config.preference);
        let mut attempts = Vec::new();
        let mut failures = Vec::new();

        for backend in order {
            let backend_impl: &dyn GraphicsBackend = match backend {
                Backend::Impeller => &self.impeller,
                Backend::Skia => &self.skia,
            };
            let probe = backend_impl.probe(platform);
            if probe.available {
                attempts.push(BackendAttempt {
                    backend,
                    reason: None,
                });
                zeno_runtime_log!(
                    info,
                    op = "resolve_backend",
                    status = "success",
                    backend = ?backend,
                    platform = ?platform,
                    attempts = attempts.len(),
                    fallback_used = attempts.len() > 1,
                    "backend resolved"
                );
                return Ok(ResolvedBackend {
                    backend_kind: backend,
                    attempts,
                });
            }

            let reason = probe.reason.unwrap_or(BackendUnavailableReason::runtime_probe_failed(
                ZenoErrorCode::BackendProbeUnavailableWithoutReason,
                "probe_backend",
                "backend probe returned unavailable without reason",
            ));
            let error = ZenoError::BackendUnavailable {
                backend,
                reason: reason.clone(),
            };
            attempts.push(BackendAttempt {
                backend,
                reason: Some(reason.clone()),
            });
            zeno_runtime_log!(
                warn,
                event = "backend_probe_failed",
                error_code = %error.error_code(),
                component = error.component(),
                op = error.operation(),
                error_kind = error.error_kind(),
                message = %error.message(),
                status = "degraded",
                backend = ?backend,
                platform = ?platform,
                attempts = attempts.len(),
                fallback_enabled = config.allow_fallback,
                "backend probe failed"
            );
            failures.push((backend, reason));

            if !config.allow_fallback {
                break;
            }
        }

        if failures.len() == 1 {
            let (backend, reason) = failures.remove(0);
            let error = ZenoError::BackendUnavailable {
                backend,
                reason: reason.clone(),
            };
            zeno_runtime_log!(
                error,
                event = "backend_resolution_failed",
                error_code = %error.error_code(),
                component = error.component(),
                op = error.operation(),
                error_kind = error.error_kind(),
                message = %error.message(),
                status = "fail",
                backend = ?backend,
                platform = ?platform,
                attempts = attempts.len(),
                fallback_enabled = config.allow_fallback,
                "backend resolution failed"
            );
            Err(error)
        } else {
            let error = ZenoError::NoBackendAvailable {
                attempts: failures.clone(),
            };
            zeno_runtime_log!(
                error,
                event = "backend_resolution_failed",
                error_code = %error.error_code(),
                component = error.component(),
                op = error.operation(),
                error_kind = error.error_kind(),
                message = %error.message(),
                status = "fail",
                platform = ?platform,
                attempts = attempts.len(),
                fallback_enabled = config.allow_fallback,
                failure_count = failures.len(),
                "backend resolution failed"
            );
            Err(error)
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
                .unwrap_or(BackendUnavailableReason::runtime_probe_failed(
                    ZenoErrorCode::BackendProbeUnavailableWithoutReason,
                    "probe_backend",
                    "backend probe returned unavailable without reason",
                )));
        }
        backend_impl.create_renderer().map_err(|error| match error {
            ZenoError::BackendUnavailable { reason, .. } => reason,
            other => BackendUnavailableReason::runtime_probe_failed(
                other.error_code(),
                other.operation(),
                other.message().into_owned(),
            ),
        })
    }
}

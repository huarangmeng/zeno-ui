use std::{borrow::Cow, fmt::{Display, Formatter}};

use crate::{Backend, BackendUnavailableReason, ZenoErrorCode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZenoError {
    BackendUnavailable {
        backend: Backend,
        reason: BackendUnavailableReason,
    },
    NoBackendAvailable {
        attempts: Vec<(Backend, BackendUnavailableReason)>,
    },
    InvalidConfiguration {
        code: ZenoErrorCode,
        component: &'static str,
        operation: &'static str,
        message: String,
    },
}

impl ZenoError {
    #[must_use]
    pub fn invalid_configuration(
        code: ZenoErrorCode,
        component: &'static str,
        operation: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::InvalidConfiguration {
            code,
            component,
            operation,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn component(&self) -> &'static str {
        match self {
            Self::BackendUnavailable { .. } | Self::NoBackendAvailable { .. } => "runtime.resolver",
            Self::InvalidConfiguration { component, .. } => component,
        }
    }

    #[must_use]
    pub fn operation(&self) -> &'static str {
        match self {
            Self::BackendUnavailable { reason, .. } => reason.operation(),
            Self::NoBackendAvailable { .. } => "resolve_backend",
            Self::InvalidConfiguration { operation, .. } => operation,
        }
    }

    #[must_use]
    pub fn error_kind(&self) -> &'static str {
        match self {
            Self::BackendUnavailable { .. } => "backend_unavailable",
            Self::NoBackendAvailable { .. } => "no_backend_available",
            Self::InvalidConfiguration { .. } => "invalid_configuration",
        }
    }

    #[must_use]
    pub fn error_code(&self) -> ZenoErrorCode {
        match self {
            Self::BackendUnavailable { reason, .. } => reason.error_code(),
            Self::NoBackendAvailable { .. } => ZenoErrorCode::BackendNoAvailable,
            Self::InvalidConfiguration { code, .. } => *code,
        }
    }

    #[must_use]
    pub fn message(&self) -> Cow<'_, str> {
        match self {
            Self::BackendUnavailable { backend, reason } => {
                Cow::Owned(format!("{backend} unavailable: {reason}"))
            }
            Self::NoBackendAvailable { attempts } => {
                let summary = attempts
                    .iter()
                    .map(|(backend, reason)| format!("{backend}: {reason}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                Cow::Owned(format!("no backend available ({summary})"))
            }
            Self::InvalidConfiguration { message, .. } => Cow::Borrowed(message),
        }
    }
}

impl Display for ZenoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendUnavailable { backend, reason } => {
                write!(f, "{backend} unavailable: {reason}")
            }
            Self::NoBackendAvailable { attempts } => {
                let summary = attempts
                    .iter()
                    .map(|(backend, reason)| format!("{backend}: {reason}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "no backend available ({summary})")
            }
            Self::InvalidConfiguration {
                code: _,
                component,
                operation,
                message,
            } => write!(f, "{component}.{operation}: {message}"),
        }
    }
}

impl std::error::Error for ZenoError {}

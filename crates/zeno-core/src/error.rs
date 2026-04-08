use std::fmt::{Display, Formatter};

use crate::{Backend, BackendUnavailableReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZenoError {
    BackendUnavailable {
        backend: Backend,
        reason: BackendUnavailableReason,
    },
    NoBackendAvailable {
        attempts: Vec<(Backend, BackendUnavailableReason)>,
    },
    InvalidConfiguration(String),
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
            Self::InvalidConfiguration(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ZenoError {}

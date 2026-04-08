use std::fmt::{Display, Formatter};

use crate::{platform::Platform, ZenoErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Backend {
    Impeller,
    Skia,
}

impl Display for Backend {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Impeller => f.write_str("impeller"),
            Self::Skia => f.write_str("skia"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureFlags {
    pub gpu_rendering: bool,
    pub text_layout: bool,
    pub offscreen_rendering: bool,
    pub filters: bool,
}

impl FeatureFlags {
    #[must_use]
    pub const fn minimal() -> Self {
        Self {
            gpu_rendering: true,
            text_layout: true,
            offscreen_rendering: false,
            filters: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendPreference {
    Auto,
    PreferImpeller,
    PreferSkia,
    Force(Backend),
}

impl Default for BackendPreference {
    fn default() -> Self {
        Self::PreferImpeller
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendUnavailableReason {
    NotImplementedForPlatform,
    MissingPlatformSurface,
    MissingGpuContext,
    ExplicitlyDisabled,
    RuntimeProbeFailed {
        code: ZenoErrorCode,
        operation: &'static str,
        message: String,
    },
}

impl BackendUnavailableReason {
    #[must_use]
    pub fn runtime_probe_failed(
        code: ZenoErrorCode,
        operation: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::RuntimeProbeFailed {
            code,
            operation,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn error_code(&self) -> ZenoErrorCode {
        match self {
            Self::NotImplementedForPlatform => ZenoErrorCode::BackendNotImplementedForPlatform,
            Self::MissingPlatformSurface => ZenoErrorCode::BackendMissingPlatformSurface,
            Self::MissingGpuContext => ZenoErrorCode::BackendMissingGpuContext,
            Self::ExplicitlyDisabled => ZenoErrorCode::BackendExplicitlyDisabled,
            Self::RuntimeProbeFailed { code, .. } => *code,
        }
    }

    #[must_use]
    pub const fn operation(&self) -> &'static str {
        match self {
            Self::NotImplementedForPlatform => "probe_backend",
            Self::MissingPlatformSurface => "probe_backend",
            Self::MissingGpuContext => "probe_backend",
            Self::ExplicitlyDisabled => "probe_backend",
            Self::RuntimeProbeFailed { operation, .. } => operation,
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::NotImplementedForPlatform => "backend is not implemented for platform",
            Self::MissingPlatformSurface => "platform surface is unavailable",
            Self::MissingGpuContext => "gpu context is unavailable",
            Self::ExplicitlyDisabled => "backend is explicitly disabled",
            Self::RuntimeProbeFailed { message, .. } => message,
        }
    }
}

impl Display for BackendUnavailableReason {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformCapabilities {
    pub platform: Platform,
    pub supports_impeller: bool,
    pub supports_skia: bool,
    pub native_surface: bool,
    pub feature_flags: FeatureFlags,
}

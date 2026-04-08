use std::fmt::{Display, Formatter};

use crate::platform::Platform;

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
    RuntimeProbeFailed(String),
}

impl Display for BackendUnavailableReason {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotImplementedForPlatform => f.write_str("backend is not implemented for platform"),
            Self::MissingPlatformSurface => f.write_str("platform surface is unavailable"),
            Self::MissingGpuContext => f.write_str("gpu context is unavailable"),
            Self::ExplicitlyDisabled => f.write_str("backend is explicitly disabled"),
            Self::RuntimeProbeFailed(message) => f.write_str(message),
        }
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

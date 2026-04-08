use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendKind {
    Impeller,
    Skia,
}

impl Display for BackendKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Impeller => f.write_str("impeller"),
            Self::Skia => f.write_str("skia"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformKind {
    Windows,
    MacOS,
    Linux,
    Android,
    IOS,
    Unknown,
}

impl PlatformKind {
    #[must_use]
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOS
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "android") {
            Self::Android
        } else if cfg!(target_os = "ios") {
            Self::IOS
        } else {
            Self::Unknown
        }
    }
}

impl Display for PlatformKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Windows => f.write_str("windows"),
            Self::MacOS => f.write_str("macos"),
            Self::Linux => f.write_str("linux"),
            Self::Android => f.write_str("android"),
            Self::IOS => f.write_str("ios"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    #[must_use]
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Color {
    pub const WHITE: Self = Self::rgba(255, 255, 255, 255);
    pub const BLACK: Self = Self::rgba(0, 0, 0, 255);
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

    #[must_use]
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgba8Unorm,
    Bgra8Unorm,
}

pub type ScaleFactor = f32;

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
    Force(BackendKind),
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
    pub platform: PlatformKind,
    pub supports_impeller: bool,
    pub supports_skia: bool,
    pub native_surface: bool,
    pub feature_flags: FeatureFlags,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WindowConfig {
    pub title: String,
    pub size: Size,
    pub scale_factor: ScaleFactor,
    pub transparent: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Zeno UI".to_string(),
            size: Size::new(1280.0, 720.0),
            scale_factor: 1.0,
            transparent: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererConfig {
    pub preference: BackendPreference,
    pub allow_fallback: bool,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            preference: BackendPreference::PreferImpeller,
            allow_fallback: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub app_name: String,
    pub renderer: RendererConfig,
    pub window: WindowConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app_name: "zeno-ui".to_string(),
            renderer: RendererConfig::default(),
            window: WindowConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZenoError {
    BackendUnavailable {
        backend: BackendKind,
        reason: BackendUnavailableReason,
    },
    NoBackendAvailable {
        attempts: Vec<(BackendKind, BackendUnavailableReason)>,
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

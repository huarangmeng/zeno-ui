mod backend;
mod color;
mod config;
mod error;
mod error_code;
mod geometry;
mod logging;
mod platform;

pub use backend::{
    Backend, BackendPreference, BackendUnavailableReason, FeatureFlags, PlatformCapabilities,
};
pub use color::{Color, PixelFormat};
pub use config::{AppConfig, DebugConfig, RendererConfig, ScaleFactor, WindowConfig};
pub use error::ZenoError;
pub use error_code::ZenoErrorCode;
pub use geometry::{Point, Rect, Size, Transform2D};
pub use platform::Platform;

#[doc(hidden)]
pub use logging::__ensure_logging;
#[doc(hidden)]
pub use logging::__private_tracing;

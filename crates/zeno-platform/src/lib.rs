pub mod android;
pub mod desktop;
pub mod event;
pub mod ios;
#[cfg(any(feature = "mobile_android", feature = "mobile_ios"))]
pub mod mobile;
pub mod presenter;
pub mod session;
#[cfg(feature = "desktop_winit")]
mod desktop_session;
mod platform;
mod shell;
mod window;

pub use shell::{
    MinimalShell, NativeSurface, NativeSurfaceHostAttachment, NativeSurfaceHostRequirement,
    PlatformDescriptor, Shell, current_platform_descriptor,
};
pub use session::{BackendResolver, ResolvedSession};

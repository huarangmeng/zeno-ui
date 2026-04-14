pub mod android;
pub mod desktop;
#[cfg(feature = "desktop_winit")]
mod desktop_session;
pub mod event;
pub mod ios;
#[cfg(any(feature = "mobile_android", feature = "mobile_ios"))]
pub mod mobile;
mod platform;
pub mod presenter;
pub mod session;
mod shell;
mod window;

pub use session::{BackendResolver, ResolvedSession};
pub use shell::{
    MinimalShell, NativeSurface, NativeSurfaceHostAttachment, NativeSurfaceHostRequirement,
    PlatformDescriptor, Shell, current_platform_descriptor,
};

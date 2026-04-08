pub mod platform;
#[cfg(feature = "desktop_winit")]
mod desktop_session;
#[cfg(any(feature = "mobile_android", feature = "mobile_ios"))]
mod mobile;
mod shell;
mod window;

pub use shell::{
    current_platform_descriptor, DesktopShell, MinimalShell, NativeSurface, PlatformDescriptor,
    Shell,
};
#[cfg(any(feature = "mobile_android", feature = "mobile_ios"))]
pub use mobile::{MobilePlatform, MobileSessionBinding, MobileShell, MobileViewport};
#[cfg(feature = "mobile_android")]
pub use mobile::AndroidShell;
#[cfg(feature = "mobile_ios")]
pub use mobile::IosShell;
pub use window::{DesktopWindowHandle, ResolvedWindowRun};

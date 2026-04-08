pub mod platform;
mod shell;
mod window;

pub use shell::{
    current_platform_descriptor, DesktopShell, MinimalShell, NativeSurface, PlatformDescriptor,
    Shell,
};
pub use window::DesktopWindowHandle;

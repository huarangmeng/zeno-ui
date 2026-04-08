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
pub use mobile::{
    AndroidAttachContext, IosMetalLayerAttachContext, IosViewAttachContext, MobileAttachContext,
    MobileAttachedSession, MobileHostKind, MobilePlatform, MobilePresenterAttachment,
    MobilePresenterKind, MobileRenderSessionHandle, MobileSessionBinding, MobileShell,
    MobileViewport, BoxedMobileRenderSession, create_mobile_render_session,
};
#[cfg(feature = "mobile_android")]
pub use mobile::AndroidShell;
#[cfg(feature = "mobile_ios")]
pub use mobile::IosShell;
pub use window::{DesktopWindowHandle, ResolvedWindowRun};

#[cfg(feature = "desktop_winit")]
mod desktop_session;
#[cfg(any(feature = "mobile_android", feature = "mobile_ios"))]
mod mobile;
pub mod platform;
mod shell;
mod window;

#[cfg(feature = "mobile_android")]
pub use mobile::AndroidShell;
#[cfg(feature = "mobile_ios")]
pub use mobile::IosShell;
#[cfg(any(feature = "mobile_android", feature = "mobile_ios"))]
pub use mobile::{
    AndroidAttachContext, BoxedMobileRenderSession, IosMetalLayerAttachContext,
    IosViewAttachContext, MobileAttachContext, MobileAttachedSession, MobileHostKind,
    MobilePlatform, MobilePresenterAttachment, MobilePresenterInterface, MobilePresenterKind,
    MobileRenderSessionHandle, MobileSessionBinding, MobileShell, MobileViewport,
    create_mobile_render_session,
};
pub use shell::{
    DesktopShell, MinimalShell, NativeSurface, PlatformDescriptor, Shell,
    current_platform_descriptor,
};
pub use window::{DesktopWindowHandle, ResolvedWindowRun};

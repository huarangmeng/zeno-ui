mod protocol;
mod render_session;
mod session_plan;
mod shell_host;

pub use protocol::{
    AndroidAttachContext, IosMetalLayerAttachContext, IosViewAttachContext, MobileAttachContext,
    MobileAttachedSession, MobileHostKind, MobilePlatform, MobilePresenterAttachment,
    MobilePresenterInterface, MobilePresenterKind, MobileRenderSessionHandle, MobileSessionBinding,
    MobileViewport,
};
pub use render_session::{BoxedMobileRenderSession, create_mobile_render_session};
pub use shell_host::{AndroidShell, IosShell, MobileShell};

#[cfg(test)]
mod tests;

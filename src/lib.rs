mod ui_runtime;

pub use zeno_compose::{
    column, compose_scene, container, row, spacer, text, Axis, ComposeRenderer, ComposeStats,
    EdgeInsets, Node, NodeId, NodeKind, Style, TextNode,
};
pub use zeno_core::{
    AppConfig, Backend, BackendPreference, Color, DebugConfig, PlatformCapabilities, Platform,
    Point, Rect, RendererConfig, Size, WindowConfig, ZenoError, ZenoErrorCode,
};
pub use zeno_core::{
    zeno_backend_error, zeno_backend_warn, zeno_debug, zeno_error, zeno_error_error,
    zeno_frame_log, zeno_info, zeno_runtime_error, zeno_runtime_log, zeno_runtime_warn,
    zeno_session_error, zeno_session_log, zeno_session_warn, zeno_trace, zeno_warn,
    zeno_warn_error, zeno_window_error, zeno_window_warn,
};
pub use zeno_graphics::{
    Brush, CanvasOp, DrawCommand, FrameReport, GraphicsBackend, RenderCapabilities, RenderSurface,
    RenderSession, Renderer, Scene, SceneBlock, ScenePatch, SceneSubmit, Shape,
};
pub use zeno_runtime::{
    BackendAttempt, BackendResolver, FramePhases, FrameScheduler, ResolvedSession,
};
pub use zeno_shell::{MinimalShell, NativeSurface, PlatformDescriptor, Shell};
#[cfg(any(feature = "mobile_android", feature = "mobile_ios"))]
pub use zeno_shell::{
    AndroidAttachContext, IosMetalLayerAttachContext, IosViewAttachContext, MobileAttachContext,
    MobileAttachedSession, MobileHostKind, MobilePlatform, MobilePresenterAttachment,
    MobilePresenterKind, MobileRenderSessionHandle, MobileSessionBinding, MobileShell,
    MobileViewport, BoxedMobileRenderSession, create_mobile_render_session,
};
#[cfg(feature = "mobile_android")]
pub use zeno_shell::AndroidShell;
#[cfg(feature = "mobile_ios")]
pub use zeno_shell::IosShell;
pub use zeno_text::{
    FallbackTextSystem, FontDescriptor, TextLayout, TextMetrics, TextParagraph, TextSystem,
};
pub use ui_runtime::{UiFrame, UiRuntime};

#[cfg(feature = "desktop")]
pub use zeno_shell::{DesktopShell, DesktopWindowHandle, ResolvedWindowRun};

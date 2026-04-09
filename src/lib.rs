mod ui_runtime;

pub use ui_runtime::{UiFrame, UiRuntime};
pub use zeno_compose::{
    Axis, BlendMode, ComposeRenderer, ComposeStats, DropShadow, EdgeInsets, Modifier, Modifiers,
    Node, NodeId, NodeKind, Style, TextNode, TransformOrigin, column, compose_scene, container,
    dump_layout, dump_scene, row, spacer, text,
};
pub use zeno_core::{
    AppConfig, Backend, BackendPreference, Color, DebugConfig, Platform, PlatformCapabilities,
    Point, Rect, RendererConfig, Size, Transform2D, WindowConfig, ZenoError, ZenoErrorCode,
};
pub use zeno_core::{
    zeno_backend_error, zeno_backend_warn, zeno_debug, zeno_error, zeno_error_error,
    zeno_frame_log, zeno_info, zeno_runtime_error, zeno_runtime_log, zeno_runtime_warn,
    zeno_session_error, zeno_session_log, zeno_session_warn, zeno_trace, zeno_warn,
    zeno_warn_error, zeno_window_error, zeno_window_warn,
};
pub use zeno_graphics::{
    Brush, CanvasOp, DrawCommand, FrameReport, GraphicsBackend, RenderCapabilities, RenderSession,
    RenderSurface, Renderer, Scene, SceneBlendMode, SceneBlock, SceneClip, SceneEffect, SceneLayer,
    ScenePatch, SceneSubmit, SceneTransform, Shape,
};
pub use zeno_runtime::{
    BackendAttempt, BackendResolver, FramePhases, FrameScheduler, ResolvedSession,
};
#[cfg(feature = "mobile_android")]
pub use zeno_shell::AndroidShell;
#[cfg(feature = "mobile_ios")]
pub use zeno_shell::IosShell;
#[cfg(any(feature = "mobile_android", feature = "mobile_ios"))]
pub use zeno_shell::{
    AndroidAttachContext, BoxedMobileRenderSession, IosMetalLayerAttachContext,
    IosViewAttachContext, MobileAttachContext, MobileAttachedSession, MobileHostKind,
    MobilePlatform, MobilePresenterAttachment, MobilePresenterInterface, MobilePresenterKind,
    MobileRenderSessionHandle, MobileSessionBinding, MobileShell, MobileViewport,
    create_mobile_render_session,
};
pub use zeno_shell::{MinimalShell, NativeSurface, PlatformDescriptor, Shell};
pub use zeno_text::{
    CachedTextSystem, FallbackTextShaper, FallbackTextSystem, FontDescriptor, ParagraphTextCache,
    ShapedGlyph, TextCache, TextCacheStats, TextCapabilities, TextLayout, TextMetrics,
    TextParagraph, TextParagraphKey, TextShaper, TextSystem,
};

#[cfg(feature = "desktop")]
pub use zeno_shell::{DesktopShell, DesktopWindowHandle, ResolvedWindowRun};

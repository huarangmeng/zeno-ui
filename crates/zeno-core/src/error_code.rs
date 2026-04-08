use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZenoErrorCode {
    BackendUnavailable,
    BackendNoAvailable,
    BackendNotImplementedForPlatform,
    BackendMissingPlatformSurface,
    BackendMissingGpuContext,
    BackendExplicitlyDisabled,
    BackendProbeUnknownPlatform,
    BackendProbeUnavailableWithoutReason,
    BackendRendererCreateFailed,
    BackendSkiaSurfaceCreateFailed,
    BackendImpellerShaderCompileFailed,
    BackendImpellerRenderPassAttachmentMissing,
    BackendImpellerColorPipelineFunctionMissing,
    BackendImpellerColorPipelineAttachmentMissing,
    BackendImpellerColorPipelineStateCreateFailed,
    BackendImpellerTextPipelineFunctionMissing,
    BackendImpellerTextPipelineAttachmentMissing,
    BackendImpellerTextPipelineStateCreateFailed,
    SessionCreateRenderSessionFailed,
    SessionInvalidWindowWidth,
    SessionInvalidWindowHeight,
    SessionWrapRenderTargetFailed,
    SessionSwapBuffersFailed,
    SessionNextDrawableUnavailable,
    WindowCreateEventLoopFailed,
    WindowRunAppFailed,
    WindowFeatureDisabled,
    WindowRendererUnavailable,
    GraphicsScenePatchWithoutBase,
    UiRuntimeRootNotSet,
    UiRuntimeViewportNotConfigured,
    MobileViewportInvalid,
    MobileSessionPlatformMismatch,
    MobileAttachPlatformMismatch,
}

impl ZenoErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BackendUnavailable => "backend.unavailable",
            Self::BackendNoAvailable => "backend.none_available",
            Self::BackendNotImplementedForPlatform => "backend.not_implemented_for_platform",
            Self::BackendMissingPlatformSurface => "backend.missing_platform_surface",
            Self::BackendMissingGpuContext => "backend.missing_gpu_context",
            Self::BackendExplicitlyDisabled => "backend.explicitly_disabled",
            Self::BackendProbeUnknownPlatform => "backend.probe_unknown_platform",
            Self::BackendProbeUnavailableWithoutReason => "backend.probe_unavailable_without_reason",
            Self::BackendRendererCreateFailed => "backend.renderer_create_failed",
            Self::BackendSkiaSurfaceCreateFailed => "backend.skia_surface_create_failed",
            Self::BackendImpellerShaderCompileFailed => "backend.impeller_shader_compile_failed",
            Self::BackendImpellerRenderPassAttachmentMissing => {
                "backend.impeller_render_pass_attachment_missing"
            }
            Self::BackendImpellerColorPipelineFunctionMissing => {
                "backend.impeller_color_pipeline_function_missing"
            }
            Self::BackendImpellerColorPipelineAttachmentMissing => {
                "backend.impeller_color_pipeline_attachment_missing"
            }
            Self::BackendImpellerColorPipelineStateCreateFailed => {
                "backend.impeller_color_pipeline_state_create_failed"
            }
            Self::BackendImpellerTextPipelineFunctionMissing => {
                "backend.impeller_text_pipeline_function_missing"
            }
            Self::BackendImpellerTextPipelineAttachmentMissing => {
                "backend.impeller_text_pipeline_attachment_missing"
            }
            Self::BackendImpellerTextPipelineStateCreateFailed => {
                "backend.impeller_text_pipeline_state_create_failed"
            }
            Self::SessionCreateRenderSessionFailed => "session.create_render_session_failed",
            Self::SessionInvalidWindowWidth => "session.invalid_window_width",
            Self::SessionInvalidWindowHeight => "session.invalid_window_height",
            Self::SessionWrapRenderTargetFailed => "session.wrap_render_target_failed",
            Self::SessionSwapBuffersFailed => "session.swap_buffers_failed",
            Self::SessionNextDrawableUnavailable => "session.next_drawable_unavailable",
            Self::WindowCreateEventLoopFailed => "window.create_event_loop_failed",
            Self::WindowRunAppFailed => "window.run_app_failed",
            Self::WindowFeatureDisabled => "window.feature_disabled",
            Self::WindowRendererUnavailable => "window.renderer_unavailable",
            Self::GraphicsScenePatchWithoutBase => "graphics.scene_patch_without_base",
            Self::UiRuntimeRootNotSet => "ui_runtime.root_not_set",
            Self::UiRuntimeViewportNotConfigured => "ui_runtime.viewport_not_configured",
            Self::MobileViewportInvalid => "mobile.viewport_invalid",
            Self::MobileSessionPlatformMismatch => "mobile.session_platform_mismatch",
            Self::MobileAttachPlatformMismatch => "mobile.attach_platform_mismatch",
        }
    }
}

impl Display for ZenoErrorCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

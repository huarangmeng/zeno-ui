use metal::{
    Device, MTLBlendFactor, MTLPixelFormat, RenderPipelineDescriptor, RenderPipelineState,
};
use zeno_core::{ZenoError, ZenoErrorCode};
use zeno_graphics::SceneBlendMode;

// 负责创建 Metal 渲染管线，避免主入口文件继续堆叠初始化细节。
pub(super) fn create_color_pipeline(
    device: &Device,
    library: &metal::Library,
) -> Result<RenderPipelineState, ZenoError> {
    let descriptor = RenderPipelineDescriptor::new();
    let vertex = library
        .get_function("color_vertex", None)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerColorPipelineFunctionMissing,
                "backend.impeller",
                "create_color_pipeline",
                error.to_string(),
            )
        })?;
    let fragment = library
        .get_function("color_fragment", None)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerColorPipelineFunctionMissing,
                "backend.impeller",
                "create_color_pipeline",
                error.to_string(),
            )
        })?;
    descriptor.set_vertex_function(Some(&vertex));
    descriptor.set_fragment_function(Some(&fragment));
    let attachment = descriptor.color_attachments().object_at(0).ok_or_else(|| {
        ZenoError::invalid_configuration(
            ZenoErrorCode::BackendImpellerColorPipelineAttachmentMissing,
            "backend.impeller",
            "create_color_pipeline",
            "missing color attachment",
        )
    })?;
    attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    attachment.set_blending_enabled(true);
    attachment.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
    attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    attachment.set_source_alpha_blend_factor(MTLBlendFactor::SourceAlpha);
    attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    device
        .new_render_pipeline_state(&descriptor)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerColorPipelineStateCreateFailed,
                "backend.impeller",
                "create_color_pipeline",
                error.to_string(),
            )
        })
}

pub(super) fn create_text_pipeline(
    device: &Device,
    library: &metal::Library,
) -> Result<RenderPipelineState, ZenoError> {
    let descriptor = RenderPipelineDescriptor::new();
    let vertex = library.get_function("text_vertex", None).map_err(|error| {
        ZenoError::invalid_configuration(
            ZenoErrorCode::BackendImpellerTextPipelineFunctionMissing,
            "backend.impeller",
            "create_text_pipeline",
            error.to_string(),
        )
    })?;
    let fragment = library
        .get_function("text_fragment", None)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerTextPipelineFunctionMissing,
                "backend.impeller",
                "create_text_pipeline",
                error.to_string(),
            )
        })?;
    descriptor.set_vertex_function(Some(&vertex));
    descriptor.set_fragment_function(Some(&fragment));
    let attachment = descriptor.color_attachments().object_at(0).ok_or_else(|| {
        ZenoError::invalid_configuration(
            ZenoErrorCode::BackendImpellerTextPipelineAttachmentMissing,
            "backend.impeller",
            "create_text_pipeline",
            "missing color attachment",
        )
    })?;
    attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    attachment.set_blending_enabled(true);
    attachment.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
    attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    attachment.set_source_alpha_blend_factor(MTLBlendFactor::SourceAlpha);
    attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
    device
        .new_render_pipeline_state(&descriptor)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerTextPipelineStateCreateFailed,
                "backend.impeller",
                "create_text_pipeline",
                error.to_string(),
            )
        })
}

pub(super) fn create_composite_pipeline(
    device: &Device,
    library: &metal::Library,
    blend_mode: SceneBlendMode,
) -> Result<RenderPipelineState, ZenoError> {
    let descriptor = RenderPipelineDescriptor::new();
    let vertex = library
        .get_function("composite_vertex", None)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerCompositePipelineFunctionMissing,
                "backend.impeller",
                "create_composite_pipeline",
                error.to_string(),
            )
        })?;
    let fragment = library
        .get_function("composite_fragment", None)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerCompositePipelineFunctionMissing,
                "backend.impeller",
                "create_composite_pipeline",
                error.to_string(),
            )
        })?;
    descriptor.set_vertex_function(Some(&vertex));
    descriptor.set_fragment_function(Some(&fragment));
    let attachment = descriptor.color_attachments().object_at(0).ok_or_else(|| {
        ZenoError::invalid_configuration(
            ZenoErrorCode::BackendImpellerCompositePipelineAttachmentMissing,
            "backend.impeller",
            "create_composite_pipeline",
            "missing color attachment",
        )
    })?;
    attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    attachment.set_blending_enabled(true);
    match blend_mode {
        SceneBlendMode::Normal => {
            attachment.set_source_rgb_blend_factor(MTLBlendFactor::SourceAlpha);
            attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
            attachment.set_source_alpha_blend_factor(MTLBlendFactor::SourceAlpha);
            attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
        }
        SceneBlendMode::Multiply => {
            attachment.set_source_rgb_blend_factor(MTLBlendFactor::DestinationColor);
            attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
            attachment.set_source_alpha_blend_factor(MTLBlendFactor::One);
            attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
        }
        SceneBlendMode::Screen => {
            attachment.set_source_rgb_blend_factor(MTLBlendFactor::One);
            attachment.set_destination_rgb_blend_factor(MTLBlendFactor::OneMinusSourceColor);
            attachment.set_source_alpha_blend_factor(MTLBlendFactor::One);
            attachment.set_destination_alpha_blend_factor(MTLBlendFactor::OneMinusSourceAlpha);
        }
    }
    device
        .new_render_pipeline_state(&descriptor)
        .map_err(|error| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::BackendImpellerCompositePipelineStateCreateFailed,
                "backend.impeller",
                "create_composite_pipeline",
                error.to_string(),
            )
        })
}

mod display_list_renderer;
mod draw;
mod offscreen;
mod pipeline;
mod scissor;
mod shaders;
mod text;

use fontdue::Font;
use metal::{CommandQueue, CompileOptions, Device, MetalDrawableRef, RenderPipelineState};
use shaders::SHADERS;
use zeno_core::{Color, Rect, ZenoError, ZenoErrorCode};
use zeno_scene::SceneBlendMode;
use zeno_text::{GlyphRasterCache, load_system_font};

use self::{
    display_list_renderer::render_display_list_to_drawable_region_with_load,
    pipeline::{create_color_pipeline, create_composite_pipeline, create_text_pipeline},
};

// 对外只暴露渲染器入口，具体的绘制、裁剪、离屏与管线初始化细节下沉到子模块。
pub struct MetalSceneRenderer {
    device: Device,
    queue: CommandQueue,
    color_pipeline: RenderPipelineState,
    text_pipeline: RenderPipelineState,
    composite_pipeline: RenderPipelineState,
    composite_multiply_pipeline: RenderPipelineState,
    composite_screen_pipeline: RenderPipelineState,
    font: Option<Font>,
    glyph_cache: GlyphRasterCache,
}

impl MetalSceneRenderer {
    pub fn new(device: Device, queue: CommandQueue) -> Result<Self, ZenoError> {
        let library = device
            .new_library_with_source(SHADERS, &CompileOptions::new())
            .map_err(|error| {
                ZenoError::invalid_configuration(
                    ZenoErrorCode::BackendImpellerShaderCompileFailed,
                    "backend.impeller",
                    "compile_shaders",
                    error.to_string(),
                )
            })?;

        Ok(Self {
            color_pipeline: create_color_pipeline(&device, &library)?,
            text_pipeline: create_text_pipeline(&device, &library)?,
            composite_pipeline: create_composite_pipeline(
                &device,
                &library,
                SceneBlendMode::Normal,
            )?,
            composite_multiply_pipeline: create_composite_pipeline(
                &device,
                &library,
                SceneBlendMode::Multiply,
            )?,
            composite_screen_pipeline: create_composite_pipeline(
                &device,
                &library,
                SceneBlendMode::Screen,
            )?,
            font: load_system_font(),
            glyph_cache: GlyphRasterCache::default(),
            device,
            queue,
        })
    }

    pub fn render_display_list_to_drawable(
        &mut self,
        drawable: &MetalDrawableRef,
        display_list: &zeno_scene::DisplayList,
        clear_color: Option<Color>,
    ) -> Result<(), ZenoError> {
        self.render_display_list_to_drawable_region_with_load(
            drawable,
            display_list,
            clear_color,
            false,
            None,
        )
    }

    pub fn render_display_list_to_drawable_region_with_load(
        &mut self,
        drawable: &MetalDrawableRef,
        display_list: &zeno_scene::DisplayList,
        clear_color: Option<Color>,
        preserve_contents: bool,
        dirty_bounds: Option<Rect>,
    ) -> Result<(), ZenoError> {
        render_display_list_to_drawable_region_with_load(
            &self.device,
            &self.queue,
            &self.color_pipeline,
            &self.text_pipeline,
            &self.composite_pipeline,
            &self.composite_multiply_pipeline,
            &self.composite_screen_pipeline,
            self.font.as_ref(),
            drawable,
            display_list,
            clear_color,
            preserve_contents,
            dirty_bounds,
            &mut self.glyph_cache,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        offscreen::{
            composite_params, local_effect_bounds, offscreen_sampling_padding,
            should_render_offscreen,
        },
        scissor::{effective_root_scissor, inverse_map_rect, rect_from_scissor},
    };
    use zeno_core::{Color, Rect, Size, Transform2D};
    use zeno_scene::{LayerObject, Scene, SceneBlendMode, SceneEffect};

    #[test]
    fn offscreen_layer_policy_skips_root_and_keeps_explicit_offscreen_layers() {
        let root = LayerObject::root(Size::new(200.0, 100.0));
        let child = LayerObject::new(
            10,
            10,
            Some(Scene::ROOT_LAYER_ID),
            1,
            Rect::new(0.0, 0.0, 40.0, 30.0),
            Rect::new(0.0, 0.0, 40.0, 30.0),
            Transform2D::identity(),
            None,
            0.5,
            SceneBlendMode::Normal,
            Vec::new(),
            true,
        );

        assert!(!should_render_offscreen(&root));
        assert!(should_render_offscreen(&child));
    }

    #[test]
    fn effect_bounds_and_params_include_blur_and_shadow() {
        let layer = LayerObject::new(
            10,
            10,
            Some(Scene::ROOT_LAYER_ID),
            1,
            Rect::new(0.0, 0.0, 40.0, 30.0),
            Rect::new(-18.0, -18.0, 76.0, 66.0),
            Transform2D::identity(),
            None,
            1.0,
            SceneBlendMode::Screen,
            vec![
                SceneEffect::Blur { sigma: 2.0 },
                SceneEffect::DropShadow {
                    dx: 4.0,
                    dy: 6.0,
                    blur: 3.0,
                    color: Color::rgba(10, 20, 30, 128),
                },
            ],
            true,
        );
        let bounds = local_effect_bounds(&layer);
        let params = composite_params(&layer, 76.0, 66.0);

        let blur_bounds = Rect::new(-6.0, -6.0, 52.0, 42.0);
        let shadow_bounds = Rect::new(-11.0, -9.0, 70.0, 60.0);
        assert_eq!(bounds, blur_bounds.union(&shadow_bounds));
        assert_eq!(params.flags, 3);
        assert_eq!(params.shadow_offset, [4.0, 6.0]);
        assert_eq!(
            params.shadow_color,
            [10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 128.0 / 255.0]
        );
        assert_eq!(offscreen_sampling_padding(&layer), 15.0);
    }

    #[test]
    fn root_scissor_uses_dirty_bounds_when_present() {
        let full = effective_root_scissor(None, 200.0, 100.0);
        let dirty = effective_root_scissor(Some(Rect::new(10.4, 20.2, 30.1, 40.6)), 200.0, 100.0);

        assert_eq!(full.x, 0);
        assert_eq!(full.y, 0);
        assert_eq!(full.width, 200);
        assert_eq!(full.height, 100);
        assert_eq!(dirty.x, 10);
        assert_eq!(dirty.y, 20);
        assert_eq!(dirty.width, 31);
        assert_eq!(dirty.height, 41);
    }

    #[test]
    fn inverse_map_rect_restores_translated_and_scaled_bounds() {
        let transform = Transform2D::translation(30.0, 10.0).then(Transform2D::scale(2.0, 4.0));
        let local = Rect::new(5.0, 2.0, 10.0, 10.0);
        let world = transform.map_rect(local);
        let local = inverse_map_rect(transform, world).expect("invertible transform");

        assert_eq!(local, Rect::new(5.0, 2.0, 10.0, 10.0));
    }

    #[test]
    fn rect_from_scissor_matches_scissor_extent() {
        let rect = rect_from_scissor(metal::MTLScissorRect {
            x: 12,
            y: 8,
            width: 40,
            height: 24,
        });

        assert_eq!(rect, Rect::new(12.0, 8.0, 40.0, 24.0));
    }
}

mod backend;
#[cfg(feature = "native_skia")]
mod canvas;
#[cfg(feature = "native_skia")]
mod renderer;
#[cfg(not(feature = "native_skia"))]
mod stub;

#[cfg(feature = "native_skia")]
pub use canvas::{
    render_display_list_region_to_canvas, render_display_list_to_canvas,
    SkiaTextCache, SkiaTextCacheStats, render_retained_scene_region_to_canvas,
    render_retained_scene_to_canvas, render_scene_region_to_canvas, render_scene_to_canvas,
};

pub use backend::SkiaBackend;

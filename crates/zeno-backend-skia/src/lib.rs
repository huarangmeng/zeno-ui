mod backend;
#[cfg(feature = "native_skia")]
mod canvas;
#[cfg(feature = "native_skia")]
mod renderer;
#[cfg(not(feature = "native_skia"))]
mod stub;

#[cfg(feature = "native_skia")]
pub use canvas::{
    SkiaImageCache, SkiaImageCacheStats, SkiaTextCache, SkiaTextCacheStats,
    render_display_list_region_to_canvas, render_display_list_tile_to_canvas,
    render_display_list_to_canvas,
};

pub use backend::SkiaBackend;

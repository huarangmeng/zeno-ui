mod backend;
#[cfg(feature = "real_skia")]
mod real;
#[cfg(not(feature = "real_skia"))]
mod stub;

#[cfg(feature = "real_skia")]
pub use real::render_scene_to_canvas;

pub use backend::SkiaBackend;

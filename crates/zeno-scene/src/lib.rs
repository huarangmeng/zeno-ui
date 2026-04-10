mod capabilities;
mod renderer;
mod scene;
mod surface;

pub use capabilities::{BackendProbe, RenderCapabilities};
pub use renderer::{GraphicsBackend, RenderSession, Renderer};
pub use scene::{
    Brush, CanvasOp, CommandRange, DrawCommand, Scene, SceneBlendMode, SceneBlock,
    SceneBlockOrder, SceneClip, SceneEffect, SceneLayer, SceneLayerOrder, ScenePatch,
    SceneResourceKey, SceneSubmit,
    SceneTransform, Shape, Stroke,
};
pub use surface::{FrameReport, RenderSurface};

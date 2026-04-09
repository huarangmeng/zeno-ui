use zeno_core::{Backend, Platform, Size};

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSurface {
    pub id: String,
    pub platform: Platform,
    pub size: Size,
    pub scale_factor: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameReport {
    pub backend: Backend,
    pub command_count: usize,
    pub resource_count: usize,
    pub block_count: usize,
    pub patch_upserts: usize,
    pub patch_removes: usize,
    pub surface_id: String,
}

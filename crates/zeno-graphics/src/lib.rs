use zeno_core::{
    BackendKind, BackendUnavailableReason, Color, PlatformKind, Point, Rect, Size, ZenoError,
};
use zeno_text::TextLayout;

#[derive(Debug, Clone, PartialEq)]
pub enum Brush {
    Solid(Color),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Rect(Rect),
    RoundedRect { rect: Rect, radius: f32 },
    Circle { center: Point, radius: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    Fill { shape: Shape, brush: Brush },
    Stroke { shape: Shape, stroke: Stroke },
    Text { position: Point, layout: TextLayout, color: Color },
    Clear(Color),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub size: Size,
    pub commands: Vec<DrawCommand>,
}

impl Scene {
    #[must_use]
    pub fn new(size: Size) -> Self {
        Self {
            size,
            commands: Vec::new(),
        }
    }

    pub fn push(&mut self, command: DrawCommand) {
        self.commands.push(command);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanvasOp {
    Save,
    Restore,
    Translate { x: f32, y: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSurface {
    pub id: String,
    pub platform: PlatformKind,
    pub size: Size,
    pub scale_factor: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderCapabilities {
    pub gpu_compositing: bool,
    pub text_shaping: bool,
    pub filters: bool,
    pub offscreen_rendering: bool,
}

impl RenderCapabilities {
    #[must_use]
    pub const fn minimal() -> Self {
        Self {
            gpu_compositing: true,
            text_shaping: true,
            filters: false,
            offscreen_rendering: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameReport {
    pub backend: BackendKind,
    pub command_count: usize,
    pub surface_id: String,
}

pub trait Renderer: Send + Sync {
    fn kind(&self) -> BackendKind;

    fn capabilities(&self) -> RenderCapabilities;

    fn render(&self, surface: &RenderSurface, scene: &Scene) -> Result<FrameReport, ZenoError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendProbe {
    pub kind: BackendKind,
    pub available: bool,
    pub reason: Option<BackendUnavailableReason>,
    pub capabilities: RenderCapabilities,
}

impl BackendProbe {
    #[must_use]
    pub fn available(kind: BackendKind, capabilities: RenderCapabilities) -> Self {
        Self {
            kind,
            available: true,
            reason: None,
            capabilities,
        }
    }

    #[must_use]
    pub fn unavailable(kind: BackendKind, reason: BackendUnavailableReason) -> Self {
        Self {
            kind,
            available: false,
            reason: Some(reason),
            capabilities: RenderCapabilities::minimal(),
        }
    }
}

pub trait GraphicsBackend: Send + Sync {
    fn kind(&self) -> BackendKind;

    fn name(&self) -> &'static str;

    fn probe(&self, platform: PlatformKind) -> BackendProbe;

    fn create_renderer(&self) -> Result<Box<dyn Renderer>, ZenoError>;
}

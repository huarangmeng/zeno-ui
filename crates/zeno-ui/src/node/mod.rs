use zeno_core::Color;
use zeno_text::FontDescriptor;

use crate::{
    modifier::{
        Alignment, Arrangement, BlendMode, CrossAxisAlignment, DropShadow, Modifier, Modifiers,
        TransformOrigin,
    },
    style::{Axis, EdgeInsets, Style},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub struct TextNode {
    pub content: String,
    pub font: FontDescriptor,
    pub font_size: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpacerNode {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Text(TextNode),
    Container(Box<Node>),
    Box { children: Vec<Node> },
    Stack { axis: Axis, children: Vec<Node> },
    Spacer(SpacerNode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub modifiers: Modifiers,
}

impl Node {
    #[must_use]
    pub fn new(id: NodeId, kind: NodeKind) -> Self {
        Self {
            id,
            kind,
            modifiers: Modifiers::new(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> NodeId {
        self.id
    }

    #[must_use]
    pub fn key(mut self, key: impl AsRef<str>) -> Self {
        self.id = NodeId(stable_node_key(key.as_ref().as_bytes()));
        self
    }

    #[must_use]
    pub fn modifier(mut self, modifier: Modifier) -> Self {
        self.modifiers.push(modifier);
        self
    }

    #[must_use]
    pub fn modifiers(mut self, modifiers: impl IntoIterator<Item = Modifier>) -> Self {
        self.modifiers.extend(modifiers);
        self
    }

    #[must_use]
    pub fn resolved_style(&self) -> Style {
        self.modifiers.resolve_style()
    }

    #[must_use]
    pub fn padding_all(self, value: f32) -> Self {
        self.modifier(Modifier::Padding(EdgeInsets::all(value)))
    }

    #[must_use]
    pub fn padding(self, padding: EdgeInsets) -> Self {
        self.modifier(Modifier::Padding(padding))
    }

    #[must_use]
    pub fn background(self, color: Color) -> Self {
        self.modifier(Modifier::Background(color))
    }

    #[must_use]
    pub fn foreground(self, color: Color) -> Self {
        self.modifier(Modifier::Foreground(color))
    }

    #[must_use]
    pub fn font_size(self, font_size: f32) -> Self {
        self.modifier(Modifier::FontSize(font_size))
    }

    #[must_use]
    pub fn corner_radius(self, radius: f32) -> Self {
        self.modifier(Modifier::CornerRadius(radius))
    }

    #[must_use]
    pub fn spacing(self, spacing: f32) -> Self {
        self.modifier(Modifier::Spacing(spacing))
    }

    #[must_use]
    pub fn width(self, width: f32) -> Self {
        self.modifier(Modifier::Width(width))
    }

    #[must_use]
    pub fn height(self, height: f32) -> Self {
        self.modifier(Modifier::Height(height))
    }

    #[must_use]
    pub fn fixed_size(self, width: f32, height: f32) -> Self {
        self.modifier(Modifier::FixedSize { width, height })
    }

    #[must_use]
    pub fn clip(self) -> Self {
        self.modifier(Modifier::ClipBounds)
    }

    #[must_use]
    pub fn clip_rounded(self, radius: f32) -> Self {
        self.modifier(Modifier::ClipRounded(radius))
    }

    #[must_use]
    pub fn translate(self, x: f32, y: f32) -> Self {
        self.modifier(Modifier::Translate { x, y })
    }

    #[must_use]
    pub fn scale(self, x: f32, y: f32) -> Self {
        self.modifier(Modifier::Scale { x, y })
    }

    #[must_use]
    pub fn scale_uniform(self, scale: f32) -> Self {
        self.scale(scale, scale)
    }

    #[must_use]
    pub fn rotate_degrees(self, degrees: f32) -> Self {
        self.modifier(Modifier::RotateDegrees(degrees))
    }

    #[must_use]
    pub fn transform_origin(self, x: f32, y: f32) -> Self {
        self.modifier(Modifier::TransformOrigin(TransformOrigin::new(x, y)))
    }

    #[must_use]
    pub fn content_alignment(self, alignment: Alignment) -> Self {
        self.modifier(Modifier::ContentAlignment(alignment))
    }

    #[must_use]
    pub fn arrangement(self, arrangement: Arrangement) -> Self {
        self.modifier(Modifier::Arrangement(arrangement))
    }

    #[must_use]
    pub fn cross_axis_alignment(self, alignment: CrossAxisAlignment) -> Self {
        self.modifier(Modifier::CrossAxisAlignment(alignment))
    }

    #[must_use]
    pub fn opacity(self, opacity: f32) -> Self {
        self.modifier(Modifier::Opacity(opacity))
    }

    #[must_use]
    pub fn layer(self) -> Self {
        self.modifier(Modifier::Layer)
    }

    #[must_use]
    pub fn blend_mode(self, mode: BlendMode) -> Self {
        self.modifier(Modifier::BlendMode(mode))
    }

    #[must_use]
    pub fn blend_multiply(self) -> Self {
        self.blend_mode(BlendMode::Multiply)
    }

    #[must_use]
    pub fn blend_screen(self) -> Self {
        self.blend_mode(BlendMode::Screen)
    }

    #[must_use]
    pub fn blur(self, sigma: f32) -> Self {
        self.modifier(Modifier::Blur(sigma))
    }

    #[must_use]
    pub fn drop_shadow(self, dx: f32, dy: f32, blur: f32, color: Color) -> Self {
        self.modifier(Modifier::DropShadow(DropShadow::new(dx, dy, blur, color)))
    }
}

fn stable_node_key(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

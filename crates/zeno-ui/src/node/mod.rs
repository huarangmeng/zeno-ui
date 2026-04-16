use zeno_core::Color;
use zeno_text::FontWeight;

use crate::{
    image::ImageSource,
    modifier::{
        ActionId, Alignment, Arrangement, BlendMode, CrossAxisAlignment, DropShadow, Modifier,
        Modifiers, TransformOrigin,
    },
    style::{Axis, EdgeInsets, Style},
    text_style::TextAlign,
    TextStyle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub struct TextNode {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpacerNode {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageNode {
    pub source: ImageSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    Text(TextNode),
    Image(ImageNode),
    Container(Box<Node>),
    Box { children: Vec<Node> },
    Stack { axis: Axis, children: Vec<Node> },
    Spacer(SpacerNode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub(crate) identity_key: Option<u64>,
    pub kind: NodeKind,
    pub modifiers: Modifiers,
}

impl Node {
    #[must_use]
    pub fn new(id: NodeId, kind: NodeKind) -> Self {
        Self {
            id,
            identity_key: None,
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
        let key_hash = stable_node_key(key.as_ref().as_bytes());
        self.id = NodeId(key_hash);
        self.identity_key = Some(key_hash);
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
    pub fn font_family(self, family: impl Into<String>) -> Self {
        self.modifier(Modifier::FontFamily(family.into()))
    }

    #[must_use]
    pub fn font_weight(self, weight: FontWeight) -> Self {
        self.modifier(Modifier::FontWeight(weight))
    }

    #[must_use]
    pub fn italic(self) -> Self {
        self.modifier(Modifier::Italic)
    }

    #[must_use]
    pub fn letter_spacing(self, spacing: f32) -> Self {
        self.modifier(Modifier::LetterSpacing(spacing))
    }

    #[must_use]
    pub fn line_height(self, height: f32) -> Self {
        self.modifier(Modifier::LineHeight(height))
    }

    #[must_use]
    pub fn text_align(self, align: TextAlign) -> Self {
        self.modifier(Modifier::TextAlign(align))
    }

    #[must_use]
    pub fn text_style(self, text_style: TextStyle) -> Self {
        self.modifier(Modifier::TextStyle(text_style))
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
    pub fn min_width(self, width: f32) -> Self {
        self.modifier(Modifier::MinWidth(width))
    }

    #[must_use]
    pub fn min_height(self, height: f32) -> Self {
        self.modifier(Modifier::MinHeight(height))
    }

    #[must_use]
    pub fn max_width(self, width: f32) -> Self {
        self.modifier(Modifier::MaxWidth(width))
    }

    #[must_use]
    pub fn max_height(self, height: f32) -> Self {
        self.modifier(Modifier::MaxHeight(height))
    }

    #[must_use]
    pub fn fixed_size(self, width: f32, height: f32) -> Self {
        self.modifier(Modifier::FixedSize { width, height })
    }

    #[must_use]
    pub fn image_rgba8(id: NodeId, width: f32, height: f32, rgba8: Vec<u8>) -> Self {
        Self::new(
            id,
            NodeKind::Image(ImageNode {
                source: ImageSource::rgba8(
                    width.max(1.0).round() as u32,
                    height.max(1.0).round() as u32,
                    rgba8,
                ),
            }),
        )
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

    #[must_use]
    pub fn action(self, action_id: ActionId) -> Self {
        self.modifier(Modifier::Action(action_id))
    }

    #[must_use]
    pub fn focusable(self) -> Self {
        self.modifier(Modifier::Focusable)
    }

    #[must_use]
    pub fn accept_text_input(self) -> Self {
        self.modifier(Modifier::AcceptTextInput)
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

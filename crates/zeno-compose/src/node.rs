use zeno_core::Color;
use zeno_text::FontDescriptor;

use crate::style::{Axis, EdgeInsets, Style};

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
    Stack { axis: Axis, children: Vec<Node> },
    Spacer(SpacerNode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub style: Style,
}

impl Node {
    #[must_use]
    pub(crate) fn new(id: NodeId, kind: NodeKind, style: Style) -> Self {
        Self { id, kind, style }
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
    pub fn padding_all(mut self, value: f32) -> Self {
        self.style.padding = EdgeInsets::all(value);
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: EdgeInsets) -> Self {
        self.style.padding = padding;
        self
    }

    #[must_use]
    pub fn background(mut self, color: Color) -> Self {
        self.style.background = Some(color);
        self
    }

    #[must_use]
    pub fn foreground(mut self, color: Color) -> Self {
        self.style.foreground = color;
        self
    }

    #[must_use]
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.style.corner_radius = radius;
        self
    }

    #[must_use]
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.style.spacing = spacing;
        self
    }

    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.style.width = Some(width);
        self
    }

    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.style.height = Some(height);
        self
    }

    #[must_use]
    pub fn font_size(mut self, font_size: f32) -> Self {
        if let NodeKind::Text(text) = &mut self.kind {
            text.font_size = font_size;
        }
        self
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

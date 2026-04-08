use zeno_core::{Point, Rect, Size};
use zeno_text::{line_box, TextLayout, TextParagraph, TextSystem};

use crate::{Axis, Node, NodeKind, SpacerNode, TextNode};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MeasuredNode {
    pub(crate) frame: Rect,
    pub(crate) kind: MeasuredKind,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MeasuredKind {
    Text(TextLayout),
    Single(Box<MeasuredNode>),
    Multiple(Vec<MeasuredNode>),
    Spacer,
}

pub(crate) fn measure_node(
    node: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
) -> MeasuredNode {
    match &node.kind {
        NodeKind::Text(text) => measure_text(node, text, origin, available, text_system),
        NodeKind::Container(child) => measure_container(node, child, origin, available, text_system),
        NodeKind::Stack { axis, children } => {
            measure_stack(node, *axis, children, origin, available, text_system)
        }
        NodeKind::Spacer(spacer) => measure_spacer(node, spacer, origin, available),
    }
}

pub(crate) fn measure_text(
    node: &Node,
    text: &TextNode,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
) -> MeasuredNode {
    let inner_available = inner_available(node, available);
    let paragraph = TextParagraph {
        text: text.content.clone(),
        font: text.font.clone(),
        font_size: text.font_size,
        max_width: inner_available.width.max(1.0),
    };
    let layout = text_system.layout(paragraph);
    let content = line_box(&layout);
    let size = finalize_size(node, available, content);
    MeasuredNode {
        frame: Rect::new(origin.x, origin.y, size.width, size.height),
        kind: MeasuredKind::Text(layout),
    }
}

fn measure_container(
    node: &Node,
    child: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
) -> MeasuredNode {
    let padding = node.style.padding;
    let child_origin = Point::new(origin.x + padding.left, origin.y + padding.top);
    let child_available = inner_available(node, available);
    let measured_child = measure_node(child, child_origin, child_available, text_system);
    let content = measured_child.frame.size;
    let size = finalize_size(node, available, content);
    MeasuredNode {
        frame: Rect::new(origin.x, origin.y, size.width, size.height),
        kind: MeasuredKind::Single(Box::new(measured_child)),
    }
}

fn measure_stack(
    node: &Node,
    axis: Axis,
    children: &[Node],
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
) -> MeasuredNode {
    let padding = node.style.padding;
    let inner = inner_available(node, available);
    let mut cursor_x = origin.x + padding.left;
    let mut cursor_y = origin.y + padding.top;
    let mut max_width: f32 = 0.0;
    let mut max_height: f32 = 0.0;
    let mut measured_children = Vec::with_capacity(children.len());

    for (index, child) in children.iter().enumerate() {
        let remaining = match axis {
            Axis::Horizontal => Size::new(
                (inner.width - (cursor_x - origin.x - padding.left)).max(0.0),
                inner.height,
            ),
            Axis::Vertical => Size::new(
                inner.width,
                (inner.height - (cursor_y - origin.y - padding.top)).max(0.0),
            ),
        };
        let child_origin = Point::new(cursor_x, cursor_y);
        let measured = measure_node(child, child_origin, remaining, text_system);

        match axis {
            Axis::Horizontal => {
                cursor_x += measured.frame.size.width;
                if index + 1 < children.len() {
                    cursor_x += node.style.spacing;
                }
                max_width = (cursor_x - origin.x - padding.left).max(max_width);
                max_height = max_height.max(measured.frame.size.height);
            }
            Axis::Vertical => {
                cursor_y += measured.frame.size.height;
                if index + 1 < children.len() {
                    cursor_y += node.style.spacing;
                }
                max_height = (cursor_y - origin.y - padding.top).max(max_height);
                max_width = max_width.max(measured.frame.size.width);
            }
        }

        measured_children.push(measured);
    }

    let content = Size::new(max_width, max_height);
    let size = finalize_size(node, available, content);
    MeasuredNode {
        frame: Rect::new(origin.x, origin.y, size.width, size.height),
        kind: MeasuredKind::Multiple(measured_children),
    }
}

pub(crate) fn measure_spacer(
    node: &Node,
    spacer: &SpacerNode,
    origin: Point,
    available: Size,
) -> MeasuredNode {
    let width = node.style.width.unwrap_or(spacer.width).min(available.width.max(0.0));
    let height = node.style.height.unwrap_or(spacer.height).min(available.height.max(0.0));
    MeasuredNode {
        frame: Rect::new(origin.x, origin.y, width, height),
        kind: MeasuredKind::Spacer,
    }
}

pub(crate) fn inner_available(node: &Node, available: Size) -> Size {
    Size::new(
        (available.width - node.style.padding.horizontal()).max(0.0),
        (available.height - node.style.padding.vertical()).max(0.0),
    )
}

pub(crate) fn finalize_size(node: &Node, available: Size, content: Size) -> Size {
    let natural = Size::new(
        content.width + node.style.padding.horizontal(),
        content.height + node.style.padding.vertical(),
    );
    Size::new(
        node.style.width.unwrap_or(natural.width).min(available.width.max(0.0)),
        node.style.height.unwrap_or(natural.height).min(available.height.max(0.0)),
    )
}

use zeno_core::{Point, Rect, Size};
use zeno_text::{TextLayout, TextParagraph, TextSystem, line_box};

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
        NodeKind::Container(child) => {
            measure_container(node, child, origin, available, text_system)
        }
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
    let inner_available = content_available(node, available);
    let style = node.resolved_style();
    let paragraph = TextParagraph {
        text: text.content.clone(),
        font: text.font.clone(),
        font_size: style.font_size.unwrap_or(text.font_size),
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
    let style = node.resolved_style();
    let padding = style.padding;
    let child_origin = Point::new(origin.x + padding.left, origin.y + padding.top);
    let child_available = content_available(node, available);
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
    let style = node.resolved_style();
    let padding = style.padding;
    let inner = content_available(node, available);
    let mut used_main = 0.0f32;
    let mut cursor_x = origin.x + padding.left;
    let mut cursor_y = origin.y + padding.top;
    let mut max_width: f32 = 0.0;
    let mut max_height: f32 = 0.0;
    let mut measured_children = Vec::with_capacity(children.len());

    for (index, child) in children.iter().enumerate() {
        let remaining = remaining_available_for_axis(inner, used_main, axis);
        let child_origin = Point::new(cursor_x, cursor_y);
        let measured = measure_node(child, child_origin, remaining, text_system);

        match axis {
            Axis::Horizontal => {
                cursor_x += measured.frame.size.width;
                if index + 1 < children.len() {
                    cursor_x += style.spacing;
                }
                used_main = cursor_x - origin.x - padding.left;
                max_width = (cursor_x - origin.x - padding.left).max(max_width);
                max_height = max_height.max(measured.frame.size.height);
            }
            Axis::Vertical => {
                cursor_y += measured.frame.size.height;
                if index + 1 < children.len() {
                    cursor_y += style.spacing;
                }
                used_main = cursor_y - origin.y - padding.top;
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
    let style = node.resolved_style();
    let width = style
        .width
        .unwrap_or(spacer.width)
        .min(available.width.max(0.0));
    let height = style
        .height
        .unwrap_or(spacer.height)
        .min(available.height.max(0.0));
    MeasuredNode {
        frame: Rect::new(origin.x, origin.y, width, height),
        kind: MeasuredKind::Spacer,
    }
}

pub(crate) fn content_available(node: &Node, available: Size) -> Size {
    let style = node.resolved_style();
    Size::new(
        (available.width - style.padding.horizontal()).max(0.0),
        (available.height - style.padding.vertical()).max(0.0),
    )
}

pub(crate) fn remaining_available_for_axis(available: Size, used_main: f32, axis: Axis) -> Size {
    match axis {
        Axis::Horizontal => Size::new((available.width - used_main).max(0.0), available.height),
        Axis::Vertical => Size::new(available.width, (available.height - used_main).max(0.0)),
    }
}

pub(crate) fn finalize_size(node: &Node, available: Size, content: Size) -> Size {
    let style = node.resolved_style();
    let natural = Size::new(
        content.width + style.padding.horizontal(),
        content.height + style.padding.vertical(),
    );
    Size::new(
        style
            .width
            .unwrap_or(natural.width)
            .min(available.width.max(0.0)),
        style
            .height
            .unwrap_or(natural.height)
            .min(available.height.max(0.0)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EdgeInsets, container, row, spacer};
    use zeno_core::Size;

    #[test]
    fn content_available_subtracts_padding_once() {
        let node = container(spacer(10.0, 10.0)).padding(EdgeInsets {
            left: 4.0,
            top: 6.0,
            right: 8.0,
            bottom: 10.0,
        });

        let available = content_available(&node, Size::new(80.0, 50.0));

        assert_eq!(available, Size::new(68.0, 34.0));
    }

    #[test]
    fn remaining_available_for_axis_clamps_on_main_axis() {
        assert_eq!(
            remaining_available_for_axis(Size::new(40.0, 20.0), 55.0, Axis::Horizontal),
            Size::new(0.0, 20.0)
        );
        assert_eq!(
            remaining_available_for_axis(Size::new(40.0, 20.0), 55.0, Axis::Vertical),
            Size::new(40.0, 0.0)
        );
    }

    #[test]
    fn measure_stack_uses_shared_remaining_available_logic() {
        let node = row(vec![
            spacer(30.0, 10.0),
            spacer(30.0, 10.0),
            spacer(30.0, 10.0),
        ])
        .padding_all(5.0)
        .spacing(7.0);

        let measured = measure_node(
            &node,
            Point::new(0.0, 0.0),
            Size::new(70.0, 40.0),
            &zeno_text::FallbackTextSystem,
        );

        let MeasuredKind::Multiple(children) = measured.kind else {
            panic!("expected stack children");
        };

        assert_eq!(children[0].frame.size.width, 30.0);
        assert_eq!(children[1].frame.size.width, 23.0);
        assert_eq!(children[2].frame.size.width, 0.0);
    }
}

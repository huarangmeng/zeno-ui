use std::sync::Arc;

use zeno_core::{Point, Rect, Size};
use zeno_text::{TextLayout, TextParagraph, TextSystem, line_box};

use crate::modifier::{CrossAxisAlignment, HorizontalAlignment, VerticalAlignment};
use crate::{Axis, Node, NodeId, NodeKind, SpacerNode, TextNode};
use crate::tree::{NodeIndexTable, RetainedComposeTree};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutSlot {
    pub(crate) frame: Rect,
    pub(crate) text_layout: Option<TextLayout>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutArena {
    index_table: Arc<NodeIndexTable>,
    slots: Vec<LayoutSlot>,
}

impl LayoutArena {
    #[must_use]
    pub fn from_measured(root: &Node, measured: &MeasuredNode) -> Self {
        let index_table = NodeIndexTable::build(root);
        let mut arena = Self::new(index_table);
        arena.collect(root, 0, measured);
        arena
    }

    #[must_use]
    pub fn slot(&self, node_id: NodeId) -> Option<&LayoutSlot> {
        self.index_table
            .index_of(node_id)
            .map(|index| &self.slots[index])
    }

    #[must_use]
    pub fn slot_at(&self, index: usize) -> &LayoutSlot {
        &self.slots[index]
    }

    #[must_use]
    pub fn frame(&self, node_id: NodeId) -> Option<Rect> {
        self.slot(node_id).map(|slot| slot.frame)
    }

    #[must_use]
    pub fn text_layout(&self, node_id: NodeId) -> Option<&TextLayout> {
        self.slot(node_id).and_then(|slot| slot.text_layout.as_ref())
    }

    #[must_use]
    pub fn index_table(&self) -> &Arc<NodeIndexTable> {
        &self.index_table
    }

    fn new(index_table: Arc<NodeIndexTable>) -> Self {
        Self {
            slots: vec![
                LayoutSlot {
                    frame: Rect::new(0.0, 0.0, 0.0, 0.0),
                    text_layout: None,
                };
                index_table.len()
            ],
            index_table,
        }
    }

    fn upsert(&mut self, index: usize, frame: Rect, text_layout: Option<TextLayout>) {
        self.slots[index] = LayoutSlot {
            frame,
            text_layout,
        };
    }

    fn shift(&mut self, index: usize, dx: f32, dy: f32) {
        let slot = &mut self.slots[index];
        slot.frame = Rect::new(
            slot.frame.origin.x + dx,
            slot.frame.origin.y + dy,
            slot.frame.size.width,
            slot.frame.size.height,
        );
    }

    fn collect(&mut self, node: &Node, index: usize, measured: &MeasuredNode) {
        self.slots[index] = LayoutSlot {
            frame: measured.frame,
            text_layout: match &measured.kind {
                MeasuredKind::Text(layout) => Some(layout.clone()),
                _ => None,
            },
        };
        match (&node.kind, &measured.kind) {
            (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
                let child_index = self.index_table.child_indices(index)[0];
                self.collect(child, child_index, measured_child);
            }
            (NodeKind::Box { children }, MeasuredKind::Multiple(measured_children))
            | (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
                let child_indices = self.index_table.child_indices(index).to_vec();
                for ((child, child_index), measured_child) in children
                    .iter()
                    .zip(child_indices.into_iter())
                    .zip(measured_children.iter())
                {
                    self.collect(child, child_index, measured_child);
                }
            }
            _ => {}
        }
    }
}

#[must_use]
pub(crate) fn measure_layout(
    root: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
) -> LayoutArena {
    let index_table = NodeIndexTable::build(root);
    let mut arena = LayoutArena::new(index_table);
    let _ = measure_into_arena(root, 0, origin, available, text_system, &mut arena);
    arena
}

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

#[derive(Debug, Clone)]
struct NodeLayoutData {
    frame: Rect,
}

pub(crate) fn measure_node(
    node: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
) -> MeasuredNode {
    let arena = measure_layout(node, origin, available, text_system);
    measured_from_layout(node, &arena)
}

fn measure_into_arena(
    node: &Node,
    index: usize,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    arena: &mut LayoutArena,
) -> NodeLayoutData {
    match &node.kind {
        NodeKind::Text(text) => {
            measure_text_into_arena(node, index, text, origin, available, text_system, arena)
        }
        NodeKind::Container(child) => {
            measure_container_into_arena(node, index, child, origin, available, text_system, arena)
        }
        NodeKind::Box { children } => {
            measure_box_into_arena(node, index, children, origin, available, text_system, arena)
        }
        NodeKind::Stack { axis, children } => {
            measure_stack_into_arena(
                node,
                index,
                *axis,
                children,
                origin,
                available,
                text_system,
                arena,
            )
        }
        NodeKind::Spacer(spacer) => {
            measure_spacer_into_arena(node, index, spacer, origin, available, arena)
        }
    }
}

fn measure_text_into_arena(
    node: &Node,
    index: usize,
    text: &TextNode,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    arena: &mut LayoutArena,
) -> NodeLayoutData {
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
    let frame = Rect::new(origin.x, origin.y, size.width, size.height);
    arena.upsert(index, frame, Some(layout.clone()));
    NodeLayoutData { frame }
}

fn measure_spacer_into_arena(
    node: &Node,
    index: usize,
    spacer: &SpacerNode,
    origin: Point,
    available: Size,
    arena: &mut LayoutArena,
) -> NodeLayoutData {
    let style = node.resolved_style();
    let width = style.width.unwrap_or(spacer.width).min(available.width.max(0.0));
    let height = style.height.unwrap_or(spacer.height).min(available.height.max(0.0));
    let frame = Rect::new(origin.x, origin.y, width, height);
    arena.upsert(index, frame, None);
    NodeLayoutData { frame }
}

fn measure_container_into_arena(
    node: &Node,
    index: usize,
    child: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    arena: &mut LayoutArena,
) -> NodeLayoutData {
    let style = node.resolved_style();
    let padding = style.padding;
    let child_origin = Point::new(origin.x + padding.left, origin.y + padding.top);
    let child_available = content_available(node, available);
    let child_index = arena.index_table.child_indices(index)[0];
    let measured_child = measure_into_arena(
        child,
        child_index,
        child_origin,
        child_available,
        text_system,
        arena,
    );
    let size = finalize_size(node, available, measured_child.frame.size);
    let frame = Rect::new(origin.x, origin.y, size.width, size.height);
    arena.upsert(index, frame, None);
    NodeLayoutData { frame }
}

fn measure_box_into_arena(
    node: &Node,
    index: usize,
    children: &[Node],
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    arena: &mut LayoutArena,
) -> NodeLayoutData {
    let style = node.resolved_style();
    let padding = style.padding;
    let content_origin = Point::new(origin.x + padding.left, origin.y + padding.top);
    let child_available = content_available(node, available);
    let mut child_layouts = Vec::with_capacity(children.len());
    let mut max_width = 0.0f32;
    let mut max_height = 0.0f32;
    let child_indices = arena.index_table.child_indices(index).to_vec();

    for (child, child_index) in children
        .iter()
        .zip(child_indices.iter().copied())
    {
        let measured = measure_into_arena(
            child,
            child_index,
            content_origin,
            child_available,
            text_system,
            arena,
        );
        max_width = max_width.max(measured.frame.size.width);
        max_height = max_height.max(measured.frame.size.height);
        child_layouts.push(measured);
    }

    let size = finalize_size(node, available, Size::new(max_width, max_height));
    let content_size = Size::new(
        (size.width - padding.horizontal()).max(0.0),
        (size.height - padding.vertical()).max(0.0),
    );
    for ((child, child_index), child_layout) in children
        .iter()
        .zip(child_indices.iter().copied())
        .zip(child_layouts.iter())
    {
        let aligned_origin = Point::new(
            content_origin.x
                + aligned_offset(
                    content_size.width,
                    child_layout.frame.size.width,
                    style.content_alignment.horizontal,
                ),
            content_origin.y
                + aligned_offset(
                    content_size.height,
                    child_layout.frame.size.height,
                    style.content_alignment.vertical,
                ),
        );
        shift_subtree_in_arena(
            child,
            child_index,
            aligned_origin.x - child_layout.frame.origin.x,
            aligned_origin.y - child_layout.frame.origin.y,
            arena,
        );
    }

    let frame = Rect::new(origin.x, origin.y, size.width, size.height);
    arena.upsert(index, frame, None);
    NodeLayoutData { frame }
}

fn measure_stack_into_arena(
    node: &Node,
    index: usize,
    axis: Axis,
    children: &[Node],
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    arena: &mut LayoutArena,
) -> NodeLayoutData {
    let style = node.resolved_style();
    let padding = style.padding;
    let inner = content_available(node, available);
    let content_origin = Point::new(origin.x + padding.left, origin.y + padding.top);
    let mut used_main = 0.0f32;
    let mut max_width = 0.0f32;
    let mut max_height = 0.0f32;
    let mut child_layouts = Vec::with_capacity(children.len());
    let child_indices = arena.index_table.child_indices(index).to_vec();

    for (child_position, (child, child_index)) in children
        .iter()
        .zip(child_indices.iter().copied())
        .enumerate()
    {
        let remaining = remaining_available_for_axis(inner, used_main, axis);
        let measured = measure_into_arena(
            child,
            child_index,
            content_origin,
            remaining,
            text_system,
            arena,
        );
        max_width = max_width.max(measured.frame.size.width);
        max_height = max_height.max(measured.frame.size.height);
        used_main += main_axis_extent(measured.frame.size, axis);
        if child_position + 1 < children.len() {
            used_main += style.spacing;
        }
        child_layouts.push(measured);
    }

    let base_main = stack_main_extent(&child_layouts, axis);
    let base_cross = stack_cross_extent(max_width, max_height, axis);
    let size = finalize_size(node, available, stack_content_size(axis, base_main, base_cross));
    let child_origins = position_stack_children(
        content_origin,
        Size::new(
            (size.width - padding.horizontal()).max(0.0),
            (size.height - padding.vertical()).max(0.0),
        ),
        &child_layouts,
        axis,
        style.spacing,
        style.arrangement,
        style.cross_axis_alignment,
    );
    for (((child, child_index), child_layout), child_origin) in children
        .iter()
        .zip(child_indices.iter().copied())
        .zip(child_layouts.iter())
        .zip(child_origins.into_iter())
    {
        shift_subtree_in_arena(
            child,
            child_index,
            child_origin.x - child_layout.frame.origin.x,
            child_origin.y - child_layout.frame.origin.y,
            arena,
        );
    }

    let frame = Rect::new(origin.x, origin.y, size.width, size.height);
    arena.upsert(index, frame, None);
    NodeLayoutData { frame }
}

fn shift_subtree_in_arena(node: &Node, index: usize, dx: f32, dy: f32, arena: &mut LayoutArena) {
    arena.shift(index, dx, dy);
    match &node.kind {
        NodeKind::Container(child) => {
            let child_index = arena.index_table.child_indices(index)[0];
            shift_subtree_in_arena(child, child_index, dx, dy, arena)
        }
        NodeKind::Box { children } | NodeKind::Stack { children, .. } => {
            let child_indices = arena.index_table.child_indices(index).to_vec();
            for (child, child_index) in children
                .iter()
                .zip(child_indices.into_iter())
            {
                shift_subtree_in_arena(child, child_index, dx, dy, arena);
            }
        }
        _ => {}
    }
}

fn measured_from_layout(node: &Node, arena: &LayoutArena) -> MeasuredNode {
    measured_from_layout_at(node, 0, arena)
}

fn measured_from_layout_at(node: &Node, index: usize, arena: &LayoutArena) -> MeasuredNode {
    let slot = arena.slot_at(index);
    let kind = match &node.kind {
        NodeKind::Text(_) => MeasuredKind::Text(
            slot.text_layout
                .clone()
                .expect("text layout should exist for text node"),
        ),
        NodeKind::Container(child) => {
            MeasuredKind::Single(Box::new(measured_from_layout_at(
                child,
                arena.index_table.child_indices(index)[0],
                arena,
            )))
        }
        NodeKind::Box { children } | NodeKind::Stack { children, .. } => MeasuredKind::Multiple(
            children
                .iter()
                .zip(arena.index_table.child_indices(index).iter().copied())
                .map(|(child, child_index)| measured_from_layout_at(child, child_index, arena))
                .collect(),
        ),
        NodeKind::Spacer(_) => MeasuredKind::Spacer,
    };
    MeasuredNode {
        frame: slot.frame,
        kind,
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

fn aligned_offset(container_extent: f32, child_extent: f32, alignment: impl IntoAlignmentAxis) -> f32 {
    alignment.resolve(container_extent, child_extent)
}

pub(crate) fn aligned_offset_for_cross_axis(
    container_extent: f32,
    child_extent: f32,
    alignment: CrossAxisAlignment,
) -> f32 {
    match alignment {
        CrossAxisAlignment::Start => 0.0,
        CrossAxisAlignment::Center => ((container_extent - child_extent) * 0.5).max(0.0),
        CrossAxisAlignment::End => (container_extent - child_extent).max(0.0),
    }
}

fn stack_main_extent(children: &[NodeLayoutData], axis: Axis) -> f32 {
    children
        .iter()
        .map(|child| main_axis_extent(child.frame.size, axis))
        .sum()
}

fn main_axis_extent(size: Size, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
    }
}

pub(crate) fn stack_cross_extent(max_width: f32, max_height: f32, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => max_height,
        Axis::Vertical => max_width,
    }
}

pub(crate) fn stack_content_size(axis: Axis, main: f32, cross: f32) -> Size {
    match axis {
        Axis::Horizontal => Size::new(main, cross),
        Axis::Vertical => Size::new(cross, main),
    }
}

pub(crate) fn arranged_gap_and_offset(
    container_main: f32,
    content_main: f32,
    child_count: usize,
    spacing: f32,
    arrangement: crate::Arrangement,
) -> (f32, f32) {
    let gaps = child_count.saturating_sub(1) as f32;
    let base_gap = spacing.max(0.0);
    let base_main = content_main + base_gap * gaps;
    let extra = (container_main - base_main).max(0.0);
    match arrangement {
        crate::Arrangement::Start => (base_gap, 0.0),
        crate::Arrangement::Center => (base_gap, extra * 0.5),
        crate::Arrangement::End => (base_gap, extra),
        crate::Arrangement::SpaceBetween if child_count > 1 => {
            (base_gap + extra / gaps.max(1.0), 0.0)
        }
        crate::Arrangement::SpaceAround if child_count > 0 => {
            let segment = extra / child_count as f32;
            (base_gap + segment, segment * 0.5)
        }
        crate::Arrangement::SpaceEvenly if child_count > 0 => {
            let segment = extra / (child_count + 1) as f32;
            (base_gap + segment, segment)
        }
        _ => (base_gap, 0.0),
    }
}

fn position_stack_children(
    content_origin: Point,
    content_size: Size,
    children: &[NodeLayoutData],
    axis: Axis,
    spacing: f32,
    arrangement: crate::Arrangement,
    cross_axis_alignment: CrossAxisAlignment,
) -> Vec<Point> {
    let content_main = stack_main_extent(children, axis);
    let container_main = match axis {
        Axis::Horizontal => content_size.width,
        Axis::Vertical => content_size.height,
    };
    let container_cross = match axis {
        Axis::Horizontal => content_size.height,
        Axis::Vertical => content_size.width,
    };
    let (gap, start_offset) = arranged_gap_and_offset(
        container_main,
        content_main,
        children.len(),
        spacing,
        arrangement,
    );
    let mut cursor = start_offset;
    let last_index = children.len().saturating_sub(1);
    let mut aligned = Vec::with_capacity(children.len());
    for (index, child) in children.iter().enumerate() {
        let (main_extent, cross_extent) = match axis {
            Axis::Horizontal => (child.frame.size.width, child.frame.size.height),
            Axis::Vertical => (child.frame.size.height, child.frame.size.width),
        };
        let cross_offset =
            aligned_offset_for_cross_axis(container_cross, cross_extent, cross_axis_alignment);
        let origin = match axis {
            Axis::Horizontal => Point::new(content_origin.x + cursor, content_origin.y + cross_offset),
            Axis::Vertical => Point::new(content_origin.x + cross_offset, content_origin.y + cursor),
        };
        aligned.push(origin);
        cursor += main_extent;
        if index < last_index {
            cursor += gap;
        }
    }
    aligned
}

trait IntoAlignmentAxis {
    fn resolve(self, container_extent: f32, child_extent: f32) -> f32;
}

#[must_use]
pub(crate) fn relayout_layout(
    node: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    retained: &RetainedComposeTree,
    layout_dirty_roots: &[usize],
) -> LayoutArena {
    let _ = retained;
    let _ = layout_dirty_roots;
    measure_layout(node, origin, available, text_system)
}

impl IntoAlignmentAxis for HorizontalAlignment {
    fn resolve(self, container_extent: f32, child_extent: f32) -> f32 {
        match self {
            Self::Start => 0.0,
            Self::Center => ((container_extent - child_extent) * 0.5).max(0.0),
            Self::End => (container_extent - child_extent).max(0.0),
        }
    }
}

impl IntoAlignmentAxis for VerticalAlignment {
    fn resolve(self, container_extent: f32, child_extent: f32) -> f32 {
        match self {
            Self::Top => 0.0,
            Self::Center => ((container_extent - child_extent) * 0.5).max(0.0),
            Self::Bottom => (container_extent - child_extent).max(0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EdgeInsets, Node, NodeId, NodeKind, SpacerNode};
    use zeno_core::Size;

    fn next_node_id() -> NodeId {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_NODE_ID: AtomicU64 = AtomicU64::new(1);
        NodeId(NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed))
    }

    fn spacer(width: f32, height: f32) -> Node {
        Node::new(next_node_id(), NodeKind::Spacer(SpacerNode { width, height }))
    }

    fn container(child: Node) -> Node {
        Node::new(next_node_id(), NodeKind::Container(Box::new(child)))
    }

    fn row(children: Vec<Node>) -> Node {
        Node::new(
            next_node_id(),
            NodeKind::Stack {
                axis: Axis::Horizontal,
                children,
            },
        )
    }

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

    #[test]
    fn arranged_gap_and_offset_centers_stack_content() {
        let (gap, start) = arranged_gap_and_offset(100.0, 30.0, 2, 10.0, crate::Arrangement::Center);
        assert_eq!(gap, 10.0);
        assert_eq!(start, 30.0);
    }
}

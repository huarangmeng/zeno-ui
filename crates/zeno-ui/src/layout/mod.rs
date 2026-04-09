use std::collections::HashMap;

use zeno_core::{Point, Rect, Size};
use zeno_text::{TextLayout, TextParagraph, TextSystem, line_box};

use crate::modifier::{CrossAxisAlignment, HorizontalAlignment, VerticalAlignment};
use crate::{Axis, Node, NodeId, NodeKind, SpacerNode, TextNode};
use crate::tree::RetainedComposeTree;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutSlot {
    pub(crate) node_id: NodeId,
    pub(crate) frame: Rect,
    pub(crate) text_layout: Option<TextLayout>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct LayoutArena {
    index_by_id: HashMap<NodeId, usize>,
    slots: Vec<LayoutSlot>,
}

impl LayoutArena {
    #[must_use]
    pub fn from_measured(root: &Node, measured: &MeasuredNode) -> Self {
        let mut arena = Self::default();
        arena.collect(root, measured);
        arena
    }

    #[must_use]
    pub fn slot(&self, node_id: NodeId) -> Option<&LayoutSlot> {
        self.index_by_id
            .get(&node_id)
            .copied()
            .map(|index| &self.slots[index])
    }

    #[must_use]
    pub fn frame(&self, node_id: NodeId) -> Option<Rect> {
        self.slot(node_id).map(|slot| slot.frame)
    }

    #[must_use]
    pub fn text_layout(&self, node_id: NodeId) -> Option<&TextLayout> {
        self.slot(node_id).and_then(|slot| slot.text_layout.as_ref())
    }

    fn collect(&mut self, node: &Node, measured: &MeasuredNode) {
        let index = self.slots.len();
        self.index_by_id.insert(node.id(), index);
        self.slots.push(LayoutSlot {
            node_id: node.id(),
            frame: measured.frame,
            text_layout: match &measured.kind {
                MeasuredKind::Text(layout) => Some(layout.clone()),
                _ => None,
            },
        });
        match (&node.kind, &measured.kind) {
            (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
                self.collect(child, measured_child);
            }
            (NodeKind::Box { children }, MeasuredKind::Multiple(measured_children))
            | (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
                for (child, measured_child) in children.iter().zip(measured_children.iter()) {
                    self.collect(child, measured_child);
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
    let measured = measure_node(root, origin, available, text_system);
    LayoutArena::from_measured(root, &measured)
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
        NodeKind::Box { children } => measure_box(node, children, origin, available, text_system),
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

fn measure_box(
    node: &Node,
    children: &[Node],
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
) -> MeasuredNode {
    let style = node.resolved_style();
    let padding = style.padding;
    let content_origin = Point::new(origin.x + padding.left, origin.y + padding.top);
    let child_available = content_available(node, available);
    let mut measured_children = Vec::with_capacity(children.len());
    let mut max_width = 0.0f32;
    let mut max_height = 0.0f32;

    for child in children {
        let measured = measure_node(child, content_origin, child_available, text_system);
        max_width = max_width.max(measured.frame.size.width);
        max_height = max_height.max(measured.frame.size.height);
        measured_children.push(measured);
    }

    let size = finalize_size(node, available, Size::new(max_width, max_height));
    let content_size = Size::new(
        (size.width - padding.horizontal()).max(0.0),
        (size.height - padding.vertical()).max(0.0),
    );
    let aligned_children = measured_children
        .into_iter()
        .map(|child| {
            let aligned_origin = Point::new(
                content_origin.x
                    + aligned_offset(
                        content_size.width,
                        child.frame.size.width,
                        style.content_alignment.horizontal,
                    ),
                content_origin.y
                    + aligned_offset(
                        content_size.height,
                        child.frame.size.height,
                        style.content_alignment.vertical,
                    ),
            );
            translate_measured_node(&child, aligned_origin)
        })
        .collect();

    MeasuredNode {
        frame: Rect::new(origin.x, origin.y, size.width, size.height),
        kind: MeasuredKind::Multiple(aligned_children),
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
    let content_origin = Point::new(origin.x + padding.left, origin.y + padding.top);
    let mut used_main = 0.0f32;
    let mut max_width: f32 = 0.0;
    let mut max_height: f32 = 0.0;
    let mut measured_children = Vec::with_capacity(children.len());

    for (index, child) in children.iter().enumerate() {
        let remaining = remaining_available_for_axis(inner, used_main, axis);
        let measured = measure_node(child, content_origin, remaining, text_system);
        max_width = max_width.max(measured.frame.size.width);
        max_height = max_height.max(measured.frame.size.height);
        used_main += match axis {
            Axis::Horizontal => measured.frame.size.width,
            Axis::Vertical => measured.frame.size.height,
        };
        if index + 1 < children.len() {
            used_main += style.spacing;
        }
        measured_children.push(measured);
    }

    let base_main = stack_main_extent(&measured_children, axis);
    let base_cross = stack_cross_extent(max_width, max_height, axis);
    let size = finalize_size(node, available, stack_content_size(axis, base_main, base_cross));
    let aligned_children = position_stack_children(
        content_origin,
        Size::new(
            (size.width - padding.horizontal()).max(0.0),
            (size.height - padding.vertical()).max(0.0),
        ),
        &measured_children,
        axis,
        style.spacing,
        style.arrangement,
        style.cross_axis_alignment,
    );
    MeasuredNode {
        frame: Rect::new(origin.x, origin.y, size.width, size.height),
        kind: MeasuredKind::Multiple(aligned_children),
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

pub(crate) fn stack_main_extent(children: &[MeasuredNode], axis: Axis) -> f32 {
    children
        .iter()
        .map(|child| match axis {
            Axis::Horizontal => child.frame.size.width,
            Axis::Vertical => child.frame.size.height,
        })
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

pub(crate) fn position_stack_children(
    content_origin: Point,
    content_size: Size,
    measured_children: &[MeasuredNode],
    axis: Axis,
    spacing: f32,
    arrangement: crate::Arrangement,
    cross_axis_alignment: CrossAxisAlignment,
) -> Vec<MeasuredNode> {
    let content_main = stack_main_extent(measured_children, axis);
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
        measured_children.len(),
        spacing,
        arrangement,
    );
    let mut cursor = start_offset;
    let last_index = measured_children.len().saturating_sub(1);
    let mut aligned = Vec::with_capacity(measured_children.len());
    for (index, child) in measured_children.iter().enumerate() {
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
        aligned.push(translate_measured_node(child, origin));
        cursor += main_extent;
        if index < last_index {
            cursor += gap;
        }
    }
    aligned
}

pub(crate) fn translate_measured_node(measured: &MeasuredNode, origin: Point) -> MeasuredNode {
    let dx = origin.x - measured.frame.origin.x;
    let dy = origin.y - measured.frame.origin.y;
    translate_measured_node_by_delta(measured, dx, dy)
}

pub(crate) fn translate_measured_node_by_delta(measured: &MeasuredNode, dx: f32, dy: f32) -> MeasuredNode {
    let frame = Rect::new(
        measured.frame.origin.x + dx,
        measured.frame.origin.y + dy,
        measured.frame.size.width,
        measured.frame.size.height,
    );
    let kind = match &measured.kind {
        MeasuredKind::Text(layout) => MeasuredKind::Text(layout.clone()),
        MeasuredKind::Single(child) => {
            MeasuredKind::Single(Box::new(translate_measured_node_by_delta(child, dx, dy)))
        }
        MeasuredKind::Multiple(children) => MeasuredKind::Multiple(
            children
                .iter()
                .map(|child| translate_measured_node_by_delta(child, dx, dy))
                .collect(),
        ),
        MeasuredKind::Spacer => MeasuredKind::Spacer,
    };
    MeasuredNode { frame, kind }
}

trait IntoAlignmentAxis {
    fn resolve(self, container_extent: f32, child_extent: f32) -> f32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutRelayoutClass {
    Reused,
    LocalOnly,
    ParentOnly,
    ParentAndFollowingSiblings,
}

#[must_use]
pub(crate) fn relayout_layout(
    node: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    retained: &RetainedComposeTree,
    layout_dirty_roots: &[NodeId],
) -> LayoutArena {
    let measured = relayout_node_internal(
        node,
        origin,
        available,
        text_system,
        retained,
        layout_dirty_roots,
        false,
    )
    .0;
    LayoutArena::from_measured(node, &measured)
}

fn relayout_node_internal(
    node: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    retained: &RetainedComposeTree,
    layout_dirty_roots: &[NodeId],
    force_relayout: bool,
) -> (MeasuredNode, bool) {
    let dirty = layout_dirty_roots.contains(&node.id());
    let descendant_dirty = retained.has_descendant_in(node.id(), layout_dirty_roots);
    let dirty_flags = retained.dirty_flags_for(node.id());
    if !force_relayout && !dirty && !descendant_dirty {
        if let (Some(slot), Some(cached_available)) =
            (retained.layout_for(node.id()), retained.available_for(node.id()))
        {
            if cached_available == available {
                let measured = measure_node(node, origin, available, text_system);
                return (measured, slot.frame.origin == origin);
            }
        }
    }

    match &node.kind {
        NodeKind::Text(text) => {
            let measured = measure_text(node, text, origin, available, text_system);
            let _ = classify_leaf_relayout(retained.layout_for(node.id()), &measured);
            (measured, false)
        }
        NodeKind::Spacer(spacer) => {
            let measured = measure_spacer(node, spacer, origin, available);
            let _ = classify_leaf_relayout(retained.layout_for(node.id()), &measured);
            (measured, false)
        }
        NodeKind::Container(child) => {
            let style = node.resolved_style();
            let previous_slot = retained.layout_for(node.id());
            let previous_child = retained.layout_for(child.id());
            let child_available = content_available(node, available);
            let (measured_child, child_reused) = relayout_node_internal(
                child,
                Point::new(origin.x + style.padding.left, origin.y + style.padding.top),
                child_available,
                text_system,
                retained,
                layout_dirty_roots,
                force_relayout || dirty,
            );
            let size = finalize_size(node, available, measured_child.frame.size);
            let measured = MeasuredNode {
                frame: Rect::new(origin.x, origin.y, size.width, size.height),
                kind: MeasuredKind::Single(Box::new(measured_child)),
            };
            let _ = classify_container_relayout(previous_slot, previous_child, &measured);
            let _ = child_reused;
            (measured, false)
        }
        NodeKind::Box { children } => {
            let child_force_relayout =
                force_relayout || (dirty && !dirty_flags.requires_order_only());
            let measured = relayout_box_internal(
                node,
                children,
                origin,
                available,
                text_system,
                retained,
                layout_dirty_roots,
                child_force_relayout,
            );
            (measured, false)
        }
        NodeKind::Stack { axis, children } => {
            let child_force_relayout =
                force_relayout || (dirty && !dirty_flags.requires_order_only());
            let measured = relayout_stack_internal(
                node,
                *axis,
                children,
                origin,
                available,
                text_system,
                retained,
                layout_dirty_roots,
                child_force_relayout,
            );
            (measured, false)
        }
    }
}

fn relayout_box_internal(
    node: &Node,
    children: &[Node],
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    retained: &RetainedComposeTree,
    layout_dirty_roots: &[NodeId],
    force_relayout: bool,
) -> MeasuredNode {
    let style = node.resolved_style();
    let content_origin = Point::new(origin.x + style.padding.left, origin.y + style.padding.top);
    let child_available = content_available(node, available);
    let mut measured_children = Vec::with_capacity(children.len());
    let mut max_width = 0.0f32;
    let mut max_height = 0.0f32;

    for child in children {
        let (measured_child, _) = relayout_node_internal(
            child,
            content_origin,
            child_available,
            text_system,
            retained,
            layout_dirty_roots,
            force_relayout,
        );
        max_width = max_width.max(measured_child.frame.size.width);
        max_height = max_height.max(measured_child.frame.size.height);
        measured_children.push(measured_child);
    }

    let final_size = finalize_size(node, available, Size::new(max_width, max_height));
    let content_size = Size::new(
        (final_size.width - style.padding.horizontal()).max(0.0),
        (final_size.height - style.padding.vertical()).max(0.0),
    );
    let aligned_children = measured_children
        .into_iter()
        .map(|child| {
            let origin = Point::new(
                content_origin.x
                    + aligned_offset(
                        content_size.width,
                        child.frame.size.width,
                        style.content_alignment.horizontal,
                    ),
                content_origin.y
                    + aligned_offset(
                        content_size.height,
                        child.frame.size.height,
                        style.content_alignment.vertical,
                    ),
            );
            translate_measured_node(&child, origin)
        })
        .collect();

    MeasuredNode {
        frame: Rect::new(origin.x, origin.y, final_size.width, final_size.height),
        kind: MeasuredKind::Multiple(aligned_children),
    }
}

fn relayout_stack_internal(
    node: &Node,
    axis: Axis,
    children: &[Node],
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    retained: &RetainedComposeTree,
    layout_dirty_roots: &[NodeId],
    force_relayout: bool,
) -> MeasuredNode {
    let style = node.resolved_style();
    let mut measured_children = Vec::with_capacity(children.len());
    let content_origin = Point::new(origin.x + style.padding.left, origin.y + style.padding.top);
    let content_available = content_available(node, available);
    let mut used_main = 0.0f32;
    let mut max_width = 0.0f32;
    let mut max_height = 0.0f32;
    let mut downstream_relayout = force_relayout;

    for (index, child) in children.iter().enumerate() {
        let remaining = remaining_available_for_axis(content_available, used_main, axis);
        let (measured_child, reused) = relayout_node_internal(
            child,
            content_origin,
            remaining,
            text_system,
            retained,
            layout_dirty_roots,
            downstream_relayout,
        );
        if !downstream_relayout && !reused {
            let previous_measured = retained.layout_for(child.id());
            let child_class = classify_stack_child_relayout(previous_measured, &measured_child, axis);
            downstream_relayout =
                matches!(child_class, LayoutRelayoutClass::ParentAndFollowingSiblings);
        }

        max_width = max_width.max(measured_child.frame.size.width);
        max_height = max_height.max(measured_child.frame.size.height);
        used_main += match axis {
            Axis::Horizontal => measured_child.frame.size.width,
            Axis::Vertical => measured_child.frame.size.height,
        };
        if index + 1 < children.len() {
            used_main += style.spacing;
        }
        measured_children.push(measured_child);
    }

    let main = stack_main_extent(&measured_children, axis);
    let cross = stack_cross_extent(max_width, max_height, axis);
    let content_size = stack_content_size(axis, main, cross);
    let final_size = finalize_size(node, available, content_size);
    let aligned_children = position_stack_children(
        content_origin,
        Size::new(
            (final_size.width - style.padding.horizontal()).max(0.0),
            (final_size.height - style.padding.vertical()).max(0.0),
        ),
        &measured_children,
        axis,
        style.spacing,
        style.arrangement,
        style.cross_axis_alignment,
    );
    MeasuredNode {
        frame: Rect::new(origin.x, origin.y, final_size.width, final_size.height),
        kind: MeasuredKind::Multiple(aligned_children),
    }
}

fn classify_leaf_relayout(
    previous: Option<&LayoutSlot>,
    current: &MeasuredNode,
) -> LayoutRelayoutClass {
    match previous {
        Some(previous) if previous.frame == current.frame => LayoutRelayoutClass::LocalOnly,
        Some(_) => LayoutRelayoutClass::ParentOnly,
        None => LayoutRelayoutClass::ParentOnly,
    }
}

fn classify_container_relayout(
    previous: Option<&LayoutSlot>,
    previous_child: Option<&LayoutSlot>,
    current: &MeasuredNode,
) -> LayoutRelayoutClass {
    let Some(previous) = previous else {
        return LayoutRelayoutClass::ParentOnly;
    };
    if previous.frame != current.frame {
        return LayoutRelayoutClass::ParentOnly;
    }
    let current_child_frame = match &current.kind {
        MeasuredKind::Single(child) => Some(child.frame),
        _ => None,
    };
    if previous_child.map(|child| child.frame) == current_child_frame {
        LayoutRelayoutClass::Reused
    } else {
        LayoutRelayoutClass::LocalOnly
    }
}

fn classify_stack_child_relayout(
    previous: Option<&LayoutSlot>,
    current: &MeasuredNode,
    axis: Axis,
) -> LayoutRelayoutClass {
    let Some(previous) = previous else {
        return LayoutRelayoutClass::ParentAndFollowingSiblings;
    };
    let previous_main = main_axis_extent(previous.frame.size, axis);
    let current_main = main_axis_extent(current.frame.size, axis);
    if (previous_main - current_main).abs() > f32::EPSILON {
        LayoutRelayoutClass::ParentAndFollowingSiblings
    } else if previous.frame == current.frame {
        LayoutRelayoutClass::LocalOnly
    } else {
        LayoutRelayoutClass::ParentOnly
    }
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

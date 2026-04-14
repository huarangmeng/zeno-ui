use zeno_core::{Point, Size};

use crate::Axis;
use crate::layout::NodeLayoutData;
use crate::modifier::{CrossAxisAlignment, HorizontalAlignment, VerticalAlignment};

// Geometry helpers shared by layout arena and work queue.

#[allow(dead_code)]
pub(crate) fn content_available(padding_h: f32, padding_v: f32, available: Size) -> Size {
    Size::new(
        (available.width - padding_h).max(0.0),
        (available.height - padding_v).max(0.0),
    )
}

#[must_use]
pub(crate) fn remaining_available_for_axis(available: Size, used_main: f32, axis: Axis) -> Size {
    match axis {
        Axis::Horizontal => Size::new((available.width - used_main).max(0.0), available.height),
        Axis::Vertical => Size::new(available.width, (available.height - used_main).max(0.0)),
    }
}

#[allow(dead_code)]
pub(crate) fn finalize_size(
    width: Option<f32>,
    height: Option<f32>,
    padding_h: f32,
    padding_v: f32,
    available: Size,
    content: Size,
) -> Size {
    let natural = Size::new(content.width + padding_h, content.height + padding_v);
    Size::new(
        width.unwrap_or(natural.width).min(available.width.max(0.0)),
        height
            .unwrap_or(natural.height)
            .min(available.height.max(0.0)),
    )
}

#[allow(dead_code)]
pub(crate) fn aligned_offset(
    container_extent: f32,
    child_extent: f32,
    alignment: impl IntoAlignmentAxis,
) -> f32 {
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

#[allow(dead_code)]
pub(crate) fn stack_main_extent(children: &[NodeLayoutData], axis: Axis) -> f32 {
    children
        .iter()
        .map(|child| main_axis_extent(child.frame.size, axis))
        .sum()
}

pub(crate) fn main_axis_extent(size: Size, axis: Axis) -> f32 {
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

#[allow(dead_code)]
pub(crate) fn position_stack_children(
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
            Axis::Horizontal => {
                Point::new(content_origin.x + cursor, content_origin.y + cross_offset)
            }
            Axis::Vertical => {
                Point::new(content_origin.x + cross_offset, content_origin.y + cursor)
            }
        };
        aligned.push(origin);
        cursor += main_extent;
        if index < last_index {
            cursor += gap;
        }
    }
    aligned
}

pub(crate) trait IntoAlignmentAxis {
    fn resolve(self, container_extent: f32, child_extent: f32) -> f32;
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

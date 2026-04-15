use std::sync::Arc;
use std::time::Instant;

use zeno_core::zeno_session_log;
use zeno_core::{Point, Rect, Size};
use zeno_text::{TextParagraph, TextSystem, line_box};

use crate::frontend::{FrontendObjectKind, FrontendObjectTable, compile_object_table};
use crate::layout::remaining_available_for_axis;
use crate::layout::work_queue::{LayoutTask, LayoutWorkQueue};
use crate::layout::{
    LayoutArena, NodeLayoutData, aligned_offset, aligned_offset_for_cross_axis,
    arranged_gap_and_offset, main_axis_extent, stack_content_size, stack_cross_extent,
};
use crate::{Axis, Node, Style};

pub(crate) fn measure_layout_workqueue(
    root: &Node,
    origin: Point,
    viewport: Size,
    text_system: &dyn TextSystem,
) -> LayoutArena {
    measure_layout_with_objects(&compile_object_table(root), origin, viewport, text_system)
}

pub(crate) fn measure_layout_with_objects(
    objects: &FrontendObjectTable,
    origin: Point,
    viewport: Size,
    text_system: &dyn TextSystem,
) -> LayoutArena {
    let mut arena = LayoutArena::new(Arc::new(objects.clone()));
    run_queue(
        objects,
        &mut arena,
        LayoutTask::Measure {
            index: 0,
            origin,
            available: viewport,
        },
        text_system,
    );
    arena
}

pub(crate) fn remeasure_subtree_with_objects(
    objects: &FrontendObjectTable,
    arena: &mut LayoutArena,
    root_index: usize,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
) {
    run_queue(
        objects,
        arena,
        LayoutTask::Measure {
            index: root_index,
            origin,
            available,
        },
        text_system,
    );
}

pub(crate) fn finalize_existing_node(
    index: usize,
    origin: Point,
    available: Size,
    objects: &FrontendObjectTable,
    arena: &mut LayoutArena,
) {
    let object = objects.object(index);
    match &object.kind {
        FrontendObjectKind::Text(_)
        | FrontendObjectKind::Image(_)
        | FrontendObjectKind::Spacer(_) => {}
        FrontendObjectKind::Container => {
            let child_index = objects.child_indices(index)[0];
            finalize_container(index, origin, available, child_index, objects, arena);
        }
        FrontendObjectKind::Box => {
            let child_indices = objects.child_indices(index).to_vec();
            finalize_box(index, origin, available, &child_indices, objects, arena);
        }
        FrontendObjectKind::Stack { axis } => {
            let child_indices = objects.child_indices(index).to_vec();
            finalize_stack(
                index,
                origin,
                available,
                &child_indices,
                *axis,
                objects,
                arena,
            );
        }
    }
}

fn run_queue(
    objects: &FrontendObjectTable,
    arena: &mut LayoutArena,
    root_task: LayoutTask,
    text_system: &dyn TextSystem,
) {
    let mut q = LayoutWorkQueue::new();
    q.push(root_task);
    while let Some(task) = q.pop() {
        match task {
            LayoutTask::Measure {
                index,
                origin,
                available,
            } => measure_task(
                index,
                origin,
                available,
                objects,
                text_system,
                arena,
                &mut q,
            ),
            LayoutTask::FinalizeContainer {
                index,
                origin,
                available,
                child_index,
            } => finalize_container(index, origin, available, child_index, objects, arena),
            LayoutTask::FinalizeBox {
                index,
                origin,
                available,
                child_indices,
            } => finalize_box(index, origin, available, &child_indices, objects, arena),
            LayoutTask::ContinueStack {
                index,
                origin,
                available,
                child_indices,
                next_child_offset,
                used_main,
            } => continue_stack(
                index,
                origin,
                available,
                &child_indices,
                next_child_offset,
                used_main,
                objects,
                arena,
                &mut q,
            ),
            LayoutTask::ResumeStack {
                index,
                origin,
                available,
                child_indices,
                measured_child_offset,
                used_main_before_child,
            } => resume_stack(
                index,
                origin,
                available,
                &child_indices,
                measured_child_offset,
                used_main_before_child,
                objects,
                arena,
                &mut q,
            ),
        }
    }
}

fn measure_task(
    index: usize,
    origin: Point,
    available: Size,
    objects: &FrontendObjectTable,
    text_system: &dyn TextSystem,
    arena: &mut LayoutArena,
    q: &mut LayoutWorkQueue,
) {
    let object = objects.object(index);
    match &object.kind {
        FrontendObjectKind::Text(text) => {
            let inner_available = content_available_for_style(&object.style, available);
            let paragraph = TextParagraph {
                text: text.content.clone(),
                font: text.font.clone(),
                font_size: object.style.font_size.unwrap_or(text.font_size),
                max_width: inner_available.width.max(1.0),
            };
            let text_layout_started = Instant::now();
            let layout = text_system.layout(paragraph);
            let text_layout_ms = text_layout_started.elapsed().as_secs_f64() * 1000.0;
            if text_layout_ms > 1.0 {
                // Stable perf instrumentation. Keep op names in sync with
                // docs/architecture/performance-debugging.md.
                // #region debug-point text-layout-node
                zeno_session_log!(
                    trace,
                    op = "text_layout_node",
                    index,
                    element_id = object.element_id.0,
                    font_size = object.style.font_size.unwrap_or(text.font_size),
                    max_width = inner_available.width.max(1.0),
                    text_len = text.content.len(),
                    text_layout_ms,
                    "text node layout timing"
                );
                // #endregion
            }
            let content = line_box(&layout);
            let size = finalize_size_for_style(&object.style, available, content);
            arena.upsert(
                index,
                Rect::new(origin.x, origin.y, size.width, size.height),
                Some(layout),
            );
        }
        FrontendObjectKind::Image(image) => {
            let (intrinsic_width, intrinsic_height) = image.source.dimensions();
            let width = object
                .style
                .width
                .unwrap_or(intrinsic_width as f32)
                .min(available.width.max(0.0));
            let height = object
                .style
                .height
                .unwrap_or(intrinsic_height as f32)
                .min(available.height.max(0.0));
            arena.upsert(index, Rect::new(origin.x, origin.y, width, height), None);
        }
        FrontendObjectKind::Spacer(spacer) => {
            let width = object
                .style
                .width
                .unwrap_or(spacer.width)
                .min(available.width.max(0.0));
            let height = object
                .style
                .height
                .unwrap_or(spacer.height)
                .min(available.height.max(0.0));
            arena.upsert(index, Rect::new(origin.x, origin.y, width, height), None);
        }
        FrontendObjectKind::Container => {
            let child_index = objects.child_indices(index)[0];
            let padding = object.style.padding;
            let child_origin = Point::new(origin.x + padding.left, origin.y + padding.top);
            let child_available = content_available_for_style(&object.style, available);
            q.push(LayoutTask::FinalizeContainer {
                index,
                origin,
                available,
                child_index,
            });
            q.push(LayoutTask::Measure {
                index: child_index,
                origin: child_origin,
                available: child_available,
            });
        }
        FrontendObjectKind::Box => {
            let child_indices = objects.child_indices(index).to_vec();
            q.push(LayoutTask::FinalizeBox {
                index,
                origin,
                available,
                child_indices: child_indices.clone(),
            });
            let child_available = content_available_for_style(&object.style, available);
            let padding = object.style.padding;
            let content_origin = Point::new(origin.x + padding.left, origin.y + padding.top);
            for child_index in child_indices.into_iter().rev() {
                q.push(LayoutTask::Measure {
                    index: child_index,
                    origin: content_origin,
                    available: child_available,
                });
            }
        }
        FrontendObjectKind::Stack { .. } => {
            q.push(LayoutTask::ContinueStack {
                index,
                origin,
                available,
                child_indices: objects.child_indices(index).to_vec(),
                next_child_offset: 0,
                used_main: 0.0,
            });
        }
    }
}

fn finalize_container(
    index: usize,
    origin: Point,
    available: Size,
    child_index: usize,
    objects: &FrontendObjectTable,
    arena: &mut LayoutArena,
) {
    let object = objects.object(index);
    let child_size = arena.slot_at(child_index).frame.size;
    let size = finalize_size_for_style(&object.style, available, child_size);
    arena.upsert(
        index,
        Rect::new(origin.x, origin.y, size.width, size.height),
        None,
    );
}

fn finalize_box(
    index: usize,
    origin: Point,
    available: Size,
    child_indices: &[usize],
    objects: &FrontendObjectTable,
    arena: &mut LayoutArena,
) {
    let object = objects.object(index);
    let padding = object.style.padding;
    let content_origin = Point::new(origin.x + padding.left, origin.y + padding.top);
    let mut child_layouts = Vec::with_capacity(child_indices.len());
    let mut max_width = 0.0f32;
    let mut max_height = 0.0f32;
    for &child_index in child_indices {
        let frame = arena.slot_at(child_index).frame;
        max_width = max_width.max(frame.size.width);
        max_height = max_height.max(frame.size.height);
        child_layouts.push(NodeLayoutData { frame });
    }
    let size = finalize_size_for_style(&object.style, available, Size::new(max_width, max_height));
    let content_size = Size::new(
        (size.width - padding.horizontal()).max(0.0),
        (size.height - padding.vertical()).max(0.0),
    );
    for (&child_index, child_layout) in child_indices.iter().zip(child_layouts.iter()) {
        let aligned_origin = Point::new(
            content_origin.x
                + aligned_offset(
                    content_size.width,
                    child_layout.frame.size.width,
                    object.style.content_alignment.horizontal,
                ),
            content_origin.y
                + aligned_offset(
                    content_size.height,
                    child_layout.frame.size.height,
                    object.style.content_alignment.vertical,
                ),
        );
        shift_subtree(
            child_index,
            objects,
            aligned_origin.x - child_layout.frame.origin.x,
            aligned_origin.y - child_layout.frame.origin.y,
            arena,
        );
    }
    arena.upsert(
        index,
        Rect::new(origin.x, origin.y, size.width, size.height),
        None,
    );
}

fn continue_stack(
    index: usize,
    origin: Point,
    available: Size,
    child_indices: &[usize],
    next_child_offset: usize,
    used_main: f32,
    objects: &FrontendObjectTable,
    arena: &mut LayoutArena,
    q: &mut LayoutWorkQueue,
) {
    let object = objects.object(index);
    let FrontendObjectKind::Stack { axis } = object.kind else {
        unreachable!("continue_stack only accepts stack objects");
    };
    if next_child_offset >= child_indices.len() {
        finalize_stack(
            index,
            origin,
            available,
            child_indices,
            axis,
            objects,
            arena,
        );
        return;
    }
    let padding = object.style.padding;
    let content_origin = Point::new(origin.x + padding.left, origin.y + padding.top);
    let inner = content_available_for_style(&object.style, available);
    let child_available = remaining_available_for_axis(inner, used_main, axis);
    let child_index = child_indices[next_child_offset];
    q.push(LayoutTask::ResumeStack {
        index,
        origin,
        available,
        child_indices: child_indices.to_vec(),
        measured_child_offset: next_child_offset,
        used_main_before_child: used_main,
    });
    q.push(LayoutTask::Measure {
        index: child_index,
        origin: content_origin,
        available: child_available,
    });
}

fn resume_stack(
    index: usize,
    origin: Point,
    available: Size,
    child_indices: &[usize],
    measured_child_offset: usize,
    used_main_before_child: f32,
    objects: &FrontendObjectTable,
    arena: &mut LayoutArena,
    q: &mut LayoutWorkQueue,
) {
    let object = objects.object(index);
    let FrontendObjectKind::Stack { axis } = object.kind else {
        unreachable!("resume_stack only accepts stack objects");
    };
    let child_index = child_indices[measured_child_offset];
    let child_frame = arena.slot_at(child_index).frame;
    let mut used_main = used_main_before_child + main_axis_extent(child_frame.size, axis);
    if measured_child_offset + 1 < child_indices.len() {
        used_main += object.style.spacing;
    }
    q.push(LayoutTask::ContinueStack {
        index,
        origin,
        available,
        child_indices: child_indices.to_vec(),
        next_child_offset: measured_child_offset + 1,
        used_main,
    });
}

fn finalize_stack(
    index: usize,
    origin: Point,
    available: Size,
    child_indices: &[usize],
    axis: Axis,
    objects: &FrontendObjectTable,
    arena: &mut LayoutArena,
) {
    let object = objects.object(index);
    let padding = object.style.padding;
    let content_origin = Point::new(origin.x + padding.left, origin.y + padding.top);
    let mut child_layouts = Vec::with_capacity(child_indices.len());
    let mut max_width = 0.0f32;
    let mut max_height = 0.0f32;
    for &child_index in child_indices {
        let frame = arena.slot_at(child_index).frame;
        max_width = max_width.max(frame.size.width);
        max_height = max_height.max(frame.size.height);
        child_layouts.push(NodeLayoutData { frame });
    }
    let base_main: f32 = child_layouts
        .iter()
        .map(|child| main_axis_extent(child.frame.size, axis))
        .sum();
    let base_cross = stack_cross_extent(max_width, max_height, axis);
    let size = finalize_size_for_style(
        &object.style,
        available,
        stack_content_size(axis, base_main, base_cross),
    );
    let content_size = Size::new(
        (size.width - padding.horizontal()).max(0.0),
        (size.height - padding.vertical()).max(0.0),
    );
    let child_origins = position_stack_children(
        content_origin,
        content_size,
        &child_layouts,
        axis,
        object.style.spacing,
        object.style.arrangement,
        object.style.cross_axis_alignment,
    );
    for ((&child_index, child_layout), child_origin) in child_indices
        .iter()
        .zip(child_layouts.iter())
        .zip(child_origins.into_iter())
    {
        shift_subtree(
            child_index,
            objects,
            child_origin.x - child_layout.frame.origin.x,
            child_origin.y - child_layout.frame.origin.y,
            arena,
        );
    }
    arena.upsert(
        index,
        Rect::new(origin.x, origin.y, size.width, size.height),
        None,
    );
}

fn shift_subtree(
    root_index: usize,
    objects: &FrontendObjectTable,
    dx: f32,
    dy: f32,
    arena: &mut LayoutArena,
) {
    let mut stack = vec![root_index];
    while let Some(index) = stack.pop() {
        arena.shift(index, dx, dy);
        for &child in objects.child_indices(index).iter().rev() {
            stack.push(child);
        }
    }
}

fn content_available_for_style(style: &Style, available: Size) -> Size {
    Size::new(
        (available.width - style.padding.horizontal()).max(0.0),
        (available.height - style.padding.vertical()).max(0.0),
    )
}

fn finalize_size_for_style(style: &Style, available: Size, content: Size) -> Size {
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

fn position_stack_children(
    content_origin: Point,
    content_size: Size,
    children: &[NodeLayoutData],
    axis: Axis,
    spacing: f32,
    arrangement: crate::Arrangement,
    cross_axis_alignment: crate::CrossAxisAlignment,
) -> Vec<Point> {
    let content_main: f32 = children
        .iter()
        .map(|child| main_axis_extent(child.frame.size, axis))
        .sum();
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
    for (position, child) in children.iter().enumerate() {
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
        if position < last_index {
            cursor += gap;
        }
    }
    aligned
}

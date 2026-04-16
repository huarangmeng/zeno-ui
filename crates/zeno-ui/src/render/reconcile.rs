//! keyed reconcile 独立成模块，方便后续继续细化 dirty reason 判断。

use super::*;
use crate::frontend::{
    ElementId, FrontendObject, FrontendObjectKind, FrontendObjectTable, compile_object_table,
};

pub(super) fn reconcile_root_change(retained: &mut RetainedComposeTree, root: &Node) {
    let previous_root = retained.root().clone();
    if previous_root.id() != root.id() && previous_root.identity_key != root.identity_key {
        retained.mark_dirty(DirtyReason::Structure);
        return;
    }

    let current_objects = compile_object_table(root);
    let previous_objects = retained.objects().clone();

    let previous_by_id: HashMap<ElementId, (usize, &FrontendObject)> = previous_objects
        .objects
        .iter()
        .enumerate()
        .map(|(index, object)| (object.element_id, (index, object)))
        .collect();
    let current_by_id: HashMap<ElementId, (usize, &FrontendObject)> = current_objects
        .objects
        .iter()
        .enumerate()
        .map(|(index, object)| (object.element_id, (index, object)))
        .collect();
    for element_id in previous_by_id.keys() {
        if !current_by_id.contains_key(element_id) {
            retained.mark_element_dirty(*element_id, DirtyReason::Structure);
        }
    }

    for (element_id, (current_index, current_object)) in &current_by_id {
        match previous_by_id.get(element_id).copied() {
            Some((previous_index, previous_object)) => {
                if let Some(reason) = local_change_reason(
                    &previous_objects,
                    previous_index,
                    previous_object,
                    &current_objects,
                    *current_index,
                    current_object,
                ) {
                    retained.mark_element_dirty(*element_id, reason);
                }
            }
            None => mark_inserted_object_dirty(retained, &current_objects, *current_index),
        }
    }
}

fn mark_inserted_object_dirty(
    retained: &mut RetainedComposeTree,
    current_objects: &FrontendObjectTable,
    current_index: usize,
) {
    let object = current_objects.object(current_index);
    let mut current = object.parent;
    while let Some(parent_index) = current {
        let parent_id = current_objects.object(parent_index).element_id;
        if retained
            .layout()
            .object_table()
            .index_of_element(parent_id)
            .is_some()
        {
            retained.mark_element_dirty(parent_id, DirtyReason::Structure);
            return;
        }
        current = current_objects.object(parent_index).parent;
    }
    retained.mark_dirty(DirtyReason::Structure);
}

fn local_change_reason(
    previous_objects: &FrontendObjectTable,
    previous_index: usize,
    previous: &FrontendObject,
    current_objects: &FrontendObjectTable,
    current_index: usize,
    current: &FrontendObject,
) -> Option<DirtyReason> {
    if previous.element_id != current.element_id {
        return Some(DirtyReason::Structure);
    }

    match (&previous.kind, &current.kind) {
        (FrontendObjectKind::Text(previous_text), FrontendObjectKind::Text(current_text)) => {
            if previous_text != current_text {
                Some(DirtyReason::Text)
            } else {
                style_change_reason(&previous.style, &current.style, true, false)
            }
        }
        (FrontendObjectKind::Image(previous_image), FrontendObjectKind::Image(current_image)) => {
            let image_changed =
                previous_image.source.resource_key() != current_image.source.resource_key();
            if image_changed {
                Some(DirtyReason::Paint)
            } else {
                style_change_reason(&previous.style, &current.style, false, false)
            }
        }
        (
            FrontendObjectKind::Spacer(previous_spacer),
            FrontendObjectKind::Spacer(current_spacer),
        ) => {
            if previous_spacer != current_spacer {
                Some(DirtyReason::Layout)
            } else {
                style_change_reason(&previous.style, &current.style, false, false)
            }
        }
        (FrontendObjectKind::Container, FrontendObjectKind::Container) => {
            let previous_children = child_ids(previous_objects, previous_index);
            let current_children = child_ids(current_objects, current_index);
            if previous_children != current_children {
                return Some(DirtyReason::Structure);
            }
            style_change_reason(&previous.style, &current.style, false, false)
        }
        (FrontendObjectKind::Box, FrontendObjectKind::Box) => {
            let previous_children = child_ids(previous_objects, previous_index);
            let current_children = child_ids(current_objects, current_index);
            if previous_children != current_children {
                if same_child_members(&previous_children, &current_children) {
                    return Some(DirtyReason::Order);
                }
                return Some(DirtyReason::Structure);
            }
            style_change_reason(&previous.style, &current.style, false, true)
        }
        (
            FrontendObjectKind::Stack {
                axis: previous_axis,
            },
            FrontendObjectKind::Stack { axis: current_axis },
        ) => {
            if previous_axis != current_axis {
                return Some(DirtyReason::Structure);
            }
            let previous_children = child_ids(previous_objects, previous_index);
            let current_children = child_ids(current_objects, current_index);
            if previous_children != current_children {
                if same_child_members(&previous_children, &current_children) {
                    return Some(DirtyReason::Order);
                }
                return Some(DirtyReason::Structure);
            }
            style_change_reason(&previous.style, &current.style, false, true)
        }
        _ => Some(DirtyReason::Structure),
    }
}

fn style_change_reason(
    previous_style: &crate::Style,
    current_style: &crate::Style,
    text_node: bool,
    stack_node: bool,
) -> Option<DirtyReason> {
    let text_layout_changed = text_node
        && (previous_style.text.font_size != current_style.text.font_size
            || previous_style.text.font != current_style.text.font
            || previous_style.text.letter_spacing != current_style.text.letter_spacing
            || previous_style.text.line_height != current_style.text.line_height
            || previous_style.text.max_lines != current_style.text.max_lines
            || previous_style.text.soft_wrap != current_style.text.soft_wrap
            || previous_style.text.overflow != current_style.text.overflow);
    let text_paint_changed = text_node
        && (previous_style.text.color != current_style.text.color
            || previous_style.text.text_align != current_style.text.text_align);
    if previous_style.padding != current_style.padding
        || previous_style.width != current_style.width
        || previous_style.height != current_style.height
        || previous_style.min_width != current_style.min_width
        || previous_style.min_height != current_style.min_height
        || previous_style.max_width != current_style.max_width
        || previous_style.max_height != current_style.max_height
        || text_layout_changed
        || (stack_node
            && (previous_style.spacing != current_style.spacing
                || previous_style.arrangement != current_style.arrangement
                || previous_style.cross_axis_alignment != current_style.cross_axis_alignment))
    {
        return Some(DirtyReason::Layout);
    }
    if previous_style.background != current_style.background
        || previous_style.corner_radius != current_style.corner_radius
        || previous_style.clip != current_style.clip
        || previous_style.transform != current_style.transform
        || previous_style.transform_origin != current_style.transform_origin
        || previous_style.opacity != current_style.opacity
        || previous_style.layer != current_style.layer
        || previous_style.blend_mode != current_style.blend_mode
        || previous_style.blur != current_style.blur
        || previous_style.drop_shadow != current_style.drop_shadow
        || text_paint_changed
    {
        return Some(DirtyReason::Paint);
    }
    if previous_style == current_style {
        return None;
    }
    if text_node || stack_node {
        return Some(DirtyReason::Layout);
    }
    None
}

fn child_ids(objects: &FrontendObjectTable, index: usize) -> Vec<ElementId> {
    objects
        .child_indices(index)
        .iter()
        .map(|child_index| objects.object(*child_index).element_id)
        .collect()
}

fn same_child_members(previous: &[ElementId], current: &[ElementId]) -> bool {
    if previous.len() != current.len() {
        return false;
    }
    let previous_ids: HashSet<ElementId> = previous.iter().copied().collect();
    let current_ids: HashSet<ElementId> = current.iter().copied().collect();
    previous_ids == current_ids
}

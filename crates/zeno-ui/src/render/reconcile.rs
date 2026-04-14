//! keyed reconcile 独立成模块，方便后续继续细化 dirty reason 判断。

use super::*;
use crate::frontend::{
    FrontendObject, FrontendObjectKind, FrontendObjectTable, compile_object_table,
};

pub(super) fn reconcile_root_change(retained: &mut RetainedComposeTree, root: &Node) {
    let previous_root = retained.root().clone();
    if previous_root.id() != root.id() {
        retained.mark_dirty(DirtyReason::Structure);
        return;
    }

    let current_objects = compile_object_table(root);
    let previous_objects = retained.objects().clone();

    let previous_by_id: HashMap<NodeId, (usize, &FrontendObject)> = previous_objects
        .objects
        .iter()
        .enumerate()
        .map(|(index, object)| (object.node_id, (index, object)))
        .collect();
    let current_by_id: HashMap<NodeId, (usize, &FrontendObject)> = current_objects
        .objects
        .iter()
        .enumerate()
        .map(|(index, object)| (object.node_id, (index, object)))
        .collect();

    for node_id in previous_by_id.keys() {
        if !current_by_id.contains_key(node_id) {
            retained.mark_node_dirty(*node_id, DirtyReason::Structure);
        }
    }

    for (node_id, (current_index, current_object)) in &current_by_id {
        match previous_by_id.get(node_id).copied() {
            Some((previous_index, previous_object)) => {
                if let Some(reason) = local_change_reason(
                    &previous_objects,
                    previous_index,
                    previous_object,
                    &current_objects,
                    *current_index,
                    current_object,
                ) {
                    retained.mark_node_dirty(*node_id, reason);
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
        let parent_id = current_objects.object(parent_index).node_id;
        if retained
            .layout()
            .object_table()
            .index_of(parent_id)
            .is_some()
        {
            retained.mark_node_dirty(parent_id, DirtyReason::Structure);
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
    if previous.node_id != current.node_id {
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
            if previous_image != current_image {
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
    if previous_style.padding != current_style.padding
        || previous_style.width != current_style.width
        || previous_style.height != current_style.height
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
        || (text_node && previous_style.foreground != current_style.foreground)
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

fn child_ids(objects: &FrontendObjectTable, index: usize) -> Vec<NodeId> {
    objects
        .child_indices(index)
        .iter()
        .map(|child_index| objects.object(*child_index).node_id)
        .collect()
}

fn same_child_members(previous: &[NodeId], current: &[NodeId]) -> bool {
    if previous.len() != current.len() {
        return false;
    }
    let previous_ids: HashSet<NodeId> = previous.iter().copied().collect();
    let current_ids: HashSet<NodeId> = current.iter().copied().collect();
    previous_ids == current_ids
}

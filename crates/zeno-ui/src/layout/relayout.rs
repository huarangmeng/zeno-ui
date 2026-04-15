use zeno_core::{Point, Size};
use zeno_text::TextSystem;

use crate::Node;
use crate::frontend::{ElementId, FrontendObjectTable, compile_object_table};
use crate::tree::RetainedComposeTree;

use super::arena::LayoutArena;
use super::work_queue::{
    finalize_existing_node, measure_layout_with_objects, remeasure_subtree_with_objects,
};

#[must_use]
pub(crate) fn relayout_layout(
    node: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    retained: &RetainedComposeTree,
    layout_dirty_roots: &[usize],
) -> LayoutArena {
    let objects = compile_object_table(node);
    let previous_objects = retained.layout().object_table().as_ref();
    let same_index_order = same_index_order(previous_objects, &objects);
    let same_element_members = same_element_members(previous_objects, &objects);
    if retained.dirty().requires_structure_rebuild() || (!same_index_order && !same_element_members)
    {
        return measure_layout_with_objects(&objects, origin, available, text_system);
    }

    let mut arena = if same_index_order {
        retained.layout().clone()
    } else {
        retained
            .layout()
            .remap(std::sync::Arc::new(objects.clone()))
    };
    let mut finalized = std::collections::HashSet::new();
    let mut dirty_roots: Vec<usize> =
        remap_dirty_roots(previous_objects, &objects, layout_dirty_roots);
    dirty_roots.extend(missing_text_layout_indices(&objects, &arena));
    dirty_roots.sort_unstable();
    dirty_roots.dedup();

    for root_index in dirty_roots {
        let root_element = objects.element_id_at(root_index);
        let root_origin = arena.slot_at(root_index).frame.origin;
        let root_available =
            available_for_element(retained, previous_objects, root_element).unwrap_or(available);
        remeasure_subtree_with_objects(
            &objects,
            &mut arena,
            root_index,
            root_origin,
            root_available,
            text_system,
        );

        let mut current = objects.object(root_index).parent;
        while let Some(index) = current {
            if !finalized.insert(index) {
                current = objects.object(index).parent;
                continue;
            }
            let element_id = objects.element_id_at(index);
            let slot_origin = arena.slot_at(index).frame.origin;
            finalize_existing_node(
                index,
                slot_origin,
                available_for_element(retained, previous_objects, element_id).unwrap_or(available),
                &objects,
                &mut arena,
            );
            current = objects.object(index).parent;
        }
    }

    arena
}

fn same_index_order(previous: &FrontendObjectTable, current: &FrontendObjectTable) -> bool {
    if previous.len() != current.len() {
        return false;
    }
    (0..previous.len()).all(|index| previous.element_id_at(index) == current.element_id_at(index))
}

fn same_element_members(previous: &FrontendObjectTable, current: &FrontendObjectTable) -> bool {
    if previous.len() != current.len() {
        return false;
    }
    let previous_ids: std::collections::HashSet<ElementId> =
        previous.element_ids().iter().copied().collect();
    let current_ids: std::collections::HashSet<ElementId> =
        current.element_ids().iter().copied().collect();
    previous_ids == current_ids
}

fn remap_dirty_roots(
    previous: &FrontendObjectTable,
    current: &FrontendObjectTable,
    layout_dirty_roots: &[usize],
) -> Vec<usize> {
    layout_dirty_roots
        .iter()
        .filter_map(|&old_index| current.index_of_element(previous.element_id_at(old_index)))
        .collect()
}

fn available_for_element(
    retained: &RetainedComposeTree,
    previous: &FrontendObjectTable,
    element_id: ElementId,
) -> Option<Size> {
    let old_index = previous.index_of_element(element_id)?;
    Some(retained.available_at(old_index))
}

fn missing_text_layout_indices(objects: &FrontendObjectTable, arena: &LayoutArena) -> Vec<usize> {
    (0..objects.len())
        .filter(|&index| {
            matches!(
                objects.object(index).kind,
                crate::frontend::FrontendObjectKind::Text(_)
            ) && arena.slot_at(index).text_layout.is_none()
        })
        .collect()
}

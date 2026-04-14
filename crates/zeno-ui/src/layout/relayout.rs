use zeno_core::{Point, Size};
use zeno_text::TextSystem;

use crate::Node;
use crate::frontend::{FrontendObjectTable, compile_object_table};
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
    if retained.dirty().requires_structure_rebuild()
        || !same_index_order(retained.layout().object_table().as_ref(), &objects)
    {
        return measure_layout_with_objects(&objects, origin, available, text_system);
    }

    let mut arena = retained.layout().clone();
    let mut finalized = std::collections::HashSet::new();
    let mut dirty_roots: Vec<usize> = layout_dirty_roots.to_vec();
    dirty_roots.sort_unstable();
    dirty_roots.dedup();

    for root_index in dirty_roots {
        let root_origin = retained.layout().slot_at(root_index).frame.origin;
        let root_available = retained.available_at(root_index);
        remeasure_subtree_with_objects(
            &objects,
            &mut arena,
            root_index,
            root_origin,
            root_available,
            text_system,
        );

        let mut current = Some(root_index);
        while let Some(index) = current.and_then(|child| retained.parent_index_of(child)) {
            if !finalized.insert(index) {
                current = Some(index);
                continue;
            }
            let slot = retained.layout().slot_at(index);
            finalize_existing_node(
                index,
                slot.frame.origin,
                retained.available_at(index),
                &objects,
                &mut arena,
            );
            current = Some(index);
        }
    }

    arena
}

fn same_index_order(previous: &FrontendObjectTable, current: &FrontendObjectTable) -> bool {
    if previous.len() != current.len() {
        return false;
    }
    (0..previous.len()).all(|index| previous.node_id_at(index) == current.node_id_at(index))
}

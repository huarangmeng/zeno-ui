use std::collections::HashSet;

use zeno_scene::{
    CompositorLayerId, CompositorLayerTree, CompositorScopeEntry, DisplayList, StackingContextId,
};

use super::lookups::RenderLookupTables;

enum ScopeEntry {
    Direct,
    ChildContext(StackingContextId),
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScopeStep {
    Direct(usize),
    ChildContext(StackingContextId),
}

#[cfg(test)]
pub(super) fn scope_steps(
    display_list: &DisplayList,
    layer_tree: Option<&CompositorLayerTree>,
    parent_context: Option<StackingContextId>,
) -> Vec<ScopeStep> {
    let render_lookups = RenderLookupTables::build(display_list);
    scope_steps_with_lookups(display_list, &render_lookups, layer_tree, parent_context)
}

pub(super) fn scope_steps_with_lookups(
    display_list: &DisplayList,
    render_lookups: &RenderLookupTables,
    layer_tree: Option<&CompositorLayerTree>,
    parent_context: Option<StackingContextId>,
) -> Vec<ScopeStep> {
    if let Some(steps) = layer_tree.and_then(|tree| {
        scope_steps_from_layer_tree_with_lookups(display_list, render_lookups, tree, parent_context)
    }) {
        return steps;
    }
    let mut rendered_children: HashSet<StackingContextId> = HashSet::new();
    let mut steps = Vec::new();
    for (item_index, item) in display_list.items.iter().enumerate() {
        match scope_entry_for_item_with_lookups(
            display_list,
            render_lookups,
            parent_context,
            item.stacking_context,
        ) {
            ScopeEntry::Skip => {}
            ScopeEntry::Direct => steps.push(ScopeStep::Direct(item_index)),
            ScopeEntry::ChildContext(child) => {
                if rendered_children.insert(child) {
                    steps.push(ScopeStep::ChildContext(child));
                }
            }
        }
    }
    steps
}

#[cfg(test)]
#[allow(dead_code)]
fn scope_steps_from_layer_tree(
    display_list: &DisplayList,
    layer_tree: &CompositorLayerTree,
    parent_context: Option<StackingContextId>,
) -> Option<Vec<ScopeStep>> {
    let render_lookups = RenderLookupTables::build(display_list);
    scope_steps_from_layer_tree_with_lookups(
        display_list,
        &render_lookups,
        layer_tree,
        parent_context,
    )
}

fn scope_steps_from_layer_tree_with_lookups(
    display_list: &DisplayList,
    render_lookups: &RenderLookupTables,
    layer_tree: &CompositorLayerTree,
    parent_context: Option<StackingContextId>,
) -> Option<Vec<ScopeStep>> {
    let layer =
        layer_for_scope_with_lookups(display_list, render_lookups, layer_tree, parent_context)?;
    Some(
        layer
            .scope_entries
            .iter()
            .filter_map(|entry| match entry {
                CompositorScopeEntry::DirectItem(item_index) => {
                    Some(ScopeStep::Direct(*item_index))
                }
                CompositorScopeEntry::ChildLayer(layer_id) => {
                    child_context_id_for_layer_with_lookups(display_list, layer_tree, *layer_id)
                        .map(ScopeStep::ChildContext)
                }
            })
            .collect(),
    )
}

#[cfg(test)]
#[allow(dead_code)]
fn layer_for_scope<'a>(
    display_list: &'a DisplayList,
    layer_tree: &'a CompositorLayerTree,
    parent_context: Option<StackingContextId>,
) -> Option<&'a zeno_scene::CompositorLayer> {
    let render_lookups = RenderLookupTables::build(display_list);
    layer_for_scope_with_lookups(display_list, &render_lookups, layer_tree, parent_context)
}

fn layer_for_scope_with_lookups<'a>(
    _display_list: &'a DisplayList,
    render_lookups: &RenderLookupTables,
    layer_tree: &'a CompositorLayerTree,
    parent_context: Option<StackingContextId>,
) -> Option<&'a zeno_scene::CompositorLayer> {
    match parent_context {
        None => layer_tree
            .layers
            .iter()
            .find(|layer| layer.layer_id == CompositorLayerId(0)),
        Some(context_id) => {
            let context_index = render_lookups.context_index(context_id)?;
            layer_tree
                .layers
                .iter()
                .find(|layer| layer.stacking_context_index == Some(context_index))
        }
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn child_context_id_for_layer(
    display_list: &DisplayList,
    layer_tree: &CompositorLayerTree,
    layer_id: CompositorLayerId,
) -> Option<StackingContextId> {
    child_context_id_for_layer_with_lookups(display_list, layer_tree, layer_id)
}

fn child_context_id_for_layer_with_lookups(
    display_list: &DisplayList,
    layer_tree: &CompositorLayerTree,
    layer_id: CompositorLayerId,
) -> Option<StackingContextId> {
    let layer = layer_tree
        .layers
        .iter()
        .find(|layer| layer.layer_id == layer_id)?;
    let context_index = layer.stacking_context_index?;
    display_list
        .stacking_contexts
        .get(context_index)
        .map(|context| context.id)
}

#[cfg(test)]
#[allow(dead_code)]
fn scope_entry_for_item(
    display_list: &DisplayList,
    parent_context: Option<StackingContextId>,
    item_context: Option<StackingContextId>,
) -> ScopeEntry {
    let render_lookups = RenderLookupTables::build(display_list);
    scope_entry_for_item_with_lookups(display_list, &render_lookups, parent_context, item_context)
}

fn scope_entry_for_item_with_lookups(
    display_list: &DisplayList,
    render_lookups: &RenderLookupTables,
    parent_context: Option<StackingContextId>,
    item_context: Option<StackingContextId>,
) -> ScopeEntry {
    match (parent_context, item_context) {
        (None, None) => ScopeEntry::Direct,
        (Some(parent), Some(current)) if current == parent => ScopeEntry::Direct,
        (scope_parent, Some(current)) => {
            match immediate_child_context_with_lookups(
                display_list,
                render_lookups,
                scope_parent,
                current,
            ) {
                Some(child) => ScopeEntry::ChildContext(child),
                None => ScopeEntry::Skip,
            }
        }
        _ => ScopeEntry::Skip,
    }
}

#[cfg(test)]
pub(super) fn immediate_child_context(
    display_list: &DisplayList,
    parent_context: Option<StackingContextId>,
    current: StackingContextId,
) -> Option<StackingContextId> {
    let render_lookups = RenderLookupTables::build(display_list);
    immediate_child_context_with_lookups(display_list, &render_lookups, parent_context, current)
}

fn immediate_child_context_with_lookups(
    display_list: &DisplayList,
    render_lookups: &RenderLookupTables,
    parent_context: Option<StackingContextId>,
    mut current: StackingContextId,
) -> Option<StackingContextId> {
    let mut path = vec![current];
    while let Some(parent) =
        parent_stacking_context_with_lookups(display_list, render_lookups, current)
    {
        path.push(parent);
        current = parent;
    }
    path.reverse();
    match parent_context {
        None => path.first().copied(),
        Some(parent) => path
            .iter()
            .position(|&id| id == parent)
            .and_then(|index| path.get(index + 1).copied()),
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn parent_stacking_context(
    display_list: &DisplayList,
    context_id: StackingContextId,
) -> Option<StackingContextId> {
    let render_lookups = RenderLookupTables::build(display_list);
    parent_stacking_context_with_lookups(display_list, &render_lookups, context_id)
}

fn parent_stacking_context_with_lookups(
    display_list: &DisplayList,
    render_lookups: &RenderLookupTables,
    context_id: StackingContextId,
) -> Option<StackingContextId> {
    let context_index = render_lookups.context_index(context_id)?;
    display_list
        .stacking_contexts
        .get(context_index)
        .and_then(|context| context.parent)
}

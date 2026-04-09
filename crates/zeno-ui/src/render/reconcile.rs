//! keyed reconcile 独立成模块，方便后续继续细化 dirty reason 判断。

use super::*;

pub(super) fn reconcile_root_change(retained: &mut RetainedComposeTree, root: &Node) {
    let previous_root = retained.root().clone();
    if previous_root.id() != root.id() {
        retained.mark_dirty(DirtyReason::Structure);
        return;
    }
    let mut previous_by_id = HashMap::new();
    index_nodes(&previous_root, &mut previous_by_id);
    let mut current_by_id = HashMap::new();
    index_nodes(root, &mut current_by_id);
    mark_removed_nodes_dirty(retained, &previous_root, &current_by_id);
    reconcile_node(retained, &previous_by_id, root);
}

fn reconcile_node<'a>(
    retained: &mut RetainedComposeTree,
    previous_by_id: &HashMap<NodeId, &'a Node>,
    current: &Node,
) {
    match previous_by_id.get(&current.id()).copied() {
        Some(previous) => {
            if let Some(reason) = local_change_reason(previous, current) {
                retained.mark_node_dirty(current.id(), reason);
            }
        }
        None => {
            retained.mark_node_dirty(current.id(), DirtyReason::Structure);
        }
    }

    match &current.kind {
        NodeKind::Container(child) => reconcile_node(retained, previous_by_id, child),
        NodeKind::Box { children } | NodeKind::Stack { children, .. } => {
            for child in children {
                reconcile_node(retained, previous_by_id, child);
            }
        }
        _ => {}
    }
}

fn mark_removed_nodes_dirty(
    retained: &mut RetainedComposeTree,
    previous: &Node,
    current_by_id: &HashMap<NodeId, &Node>,
) {
    if !current_by_id.contains_key(&previous.id()) {
        retained.mark_node_dirty(previous.id(), DirtyReason::Structure);
        return;
    }
    match &previous.kind {
        NodeKind::Container(child) => mark_removed_nodes_dirty(retained, child, current_by_id),
        NodeKind::Box { children } | NodeKind::Stack { children, .. } => {
            for child in children {
                mark_removed_nodes_dirty(retained, child, current_by_id);
            }
        }
        _ => {}
    }
}

fn index_nodes<'a>(node: &'a Node, indexed: &mut HashMap<NodeId, &'a Node>) {
    indexed.insert(node.id(), node);
    match &node.kind {
        NodeKind::Container(child) => index_nodes(child, indexed),
        NodeKind::Box { children } | NodeKind::Stack { children, .. } => {
            for child in children {
                index_nodes(child, indexed);
            }
        }
        _ => {}
    }
}

fn local_change_reason(previous: &Node, current: &Node) -> Option<DirtyReason> {
    if previous.id() != current.id() {
        return Some(DirtyReason::Structure);
    }

    match (&previous.kind, &current.kind) {
        (NodeKind::Text(previous_text), NodeKind::Text(current_text)) => {
            if previous_text != current_text {
                Some(DirtyReason::Text)
            } else {
                style_change_reason(previous, current, true, false)
            }
        }
        (NodeKind::Spacer(previous_spacer), NodeKind::Spacer(current_spacer)) => {
            if previous_spacer != current_spacer {
                Some(DirtyReason::Layout)
            } else {
                style_change_reason(previous, current, false, false)
            }
        }
        (NodeKind::Container(_), NodeKind::Container(_)) => {
            style_change_reason(previous, current, false, true)
        }
        (NodeKind::Box { children: previous_children }, NodeKind::Box { children: current_children }) => {
            if child_ids(previous_children) != child_ids(current_children) {
                if same_child_members(previous_children, current_children) {
                    return Some(DirtyReason::Order);
                }
                return Some(DirtyReason::Structure);
            }
            style_change_reason(previous, current, false, true)
        }
        (
            NodeKind::Stack {
                axis: previous_axis,
                children: previous_children,
            },
            NodeKind::Stack {
                axis: current_axis,
                children: current_children,
            },
        ) => {
            if previous_axis != current_axis {
                return Some(DirtyReason::Structure);
            }
            if child_ids(previous_children) != child_ids(current_children) {
                if same_child_members(previous_children, current_children) {
                    return Some(DirtyReason::Order);
                }
                return Some(DirtyReason::Structure);
            }
            style_change_reason(previous, current, false, true)
        }
        _ => Some(DirtyReason::Structure),
    }
}

fn style_change_reason(
    previous: &Node,
    current: &Node,
    text_node: bool,
    stack_node: bool,
) -> Option<DirtyReason> {
    let previous_style = previous.resolved_style();
    let current_style = current.resolved_style();
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

fn child_ids(children: &[Node]) -> Vec<NodeId> {
    children.iter().map(Node::id).collect()
}

fn same_child_members(previous: &[Node], current: &[Node]) -> bool {
    if previous.len() != current.len() {
        return false;
    }
    let previous_ids: HashSet<NodeId> = previous.iter().map(Node::id).collect();
    let current_ids: HashSet<NodeId> = current.iter().map(Node::id).collect();
    previous_ids == current_ids
}

use std::collections::HashMap;

use crate::{Axis, ImageNode, Node, NodeId, NodeKind, SpacerNode, Style, TextNode};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FrontendObjectTable {
    pub objects: Vec<FrontendObject>,
    index_by_id: HashMap<NodeId, usize>,
    node_ids: Vec<NodeId>,
    parents: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    container_like: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FrontendObject {
    pub node_id: NodeId,
    pub kind: FrontendObjectKind,
    pub style: Style,
    pub parent: Option<usize>,
    pub first_child: Option<usize>,
    pub next_sibling: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FrontendObjectKind {
    Text(TextNode),
    Image(ImageNode),
    Spacer(SpacerNode),
    Container,
    Box,
    Stack { axis: Axis },
}

pub(crate) fn compile_object_table(root: &Node) -> FrontendObjectTable {
    let mut index_by_id = HashMap::new();
    let mut node_ids = Vec::new();
    let mut parents = Vec::new();
    let mut children_table = Vec::new();
    let mut container_like = Vec::new();

    fn assign_indices(
        node: &Node,
        parent: Option<usize>,
        index_by_id: &mut HashMap<NodeId, usize>,
        node_ids: &mut Vec<NodeId>,
        parents: &mut Vec<Option<usize>>,
        children_table: &mut Vec<Vec<usize>>,
        container_like: &mut Vec<bool>,
    ) -> usize {
        let index = node_ids.len();
        index_by_id.insert(node.id(), index);
        node_ids.push(node.id());
        parents.push(parent);
        children_table.push(Vec::new());
        let is_container = matches!(
            node.kind,
            NodeKind::Container(_) | NodeKind::Box { .. } | NodeKind::Stack { .. }
        );
        container_like.push(is_container);

        match &node.kind {
            NodeKind::Container(child) => {
                let child_index = assign_indices(
                    child,
                    Some(index),
                    index_by_id,
                    node_ids,
                    parents,
                    children_table,
                    container_like,
                );
                children_table[index].push(child_index);
            }
            NodeKind::Box { children } | NodeKind::Stack { children, .. } => {
                for child in children {
                    let child_index = assign_indices(
                        child,
                        Some(index),
                        index_by_id,
                        node_ids,
                        parents,
                        children_table,
                        container_like,
                    );
                    children_table[index].push(child_index);
                }
            }
            _ => {}
        }
        index
    }

    assign_indices(
        root,
        None,
        &mut index_by_id,
        &mut node_ids,
        &mut parents,
        &mut children_table,
        &mut container_like,
    );

    let len = node_ids.len();
    let mut objects = Vec::with_capacity(len);
    for index in 0..len {
        let first_child = children_table[index].first().copied();
        let next_sibling = parents[index].and_then(|parent| {
            let siblings = &children_table[parent];
            siblings
                .iter()
                .position(|s| *s == index)
                .and_then(|pos| siblings.get(pos + 1).copied())
        });
        objects.push(FrontendObject {
            node_id: node_ids[index],
            kind: FrontendObjectKind::Box,
            style: Style::default(),
            parent: parents[index],
            first_child,
            next_sibling,
        });
    }
    fill_objects(root, 0, &children_table, &mut objects);

    FrontendObjectTable {
        objects,
        index_by_id,
        node_ids,
        parents,
        children: children_table,
        container_like,
    }
}

fn fill_objects(
    node: &Node,
    index: usize,
    children_table: &[Vec<usize>],
    objects: &mut [FrontendObject],
) {
    objects[index].kind = frontend_kind(&node.kind);
    objects[index].style = node.resolved_style();

    match &node.kind {
        NodeKind::Container(child) => {
            fill_objects(child, children_table[index][0], children_table, objects);
        }
        NodeKind::Box { children } | NodeKind::Stack { children, .. } => {
            for (child, &child_index) in children.iter().zip(children_table[index].iter()) {
                fill_objects(child, child_index, children_table, objects);
            }
        }
        _ => {}
    }
}

fn frontend_kind(kind: &NodeKind) -> FrontendObjectKind {
    match kind {
        NodeKind::Text(text) => FrontendObjectKind::Text(text.clone()),
        NodeKind::Image(image) => FrontendObjectKind::Image(image.clone()),
        NodeKind::Spacer(spacer) => FrontendObjectKind::Spacer(spacer.clone()),
        NodeKind::Container(_) => FrontendObjectKind::Container,
        NodeKind::Box { .. } => FrontendObjectKind::Box,
        NodeKind::Stack { axis, .. } => FrontendObjectKind::Stack { axis: *axis },
    }
}

impl FrontendObjectTable {
    #[must_use]
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    #[must_use]
    pub fn object(&self, index: usize) -> &FrontendObject {
        &self.objects[index]
    }

    #[must_use]
    pub fn child_indices(&self, index: usize) -> &[usize] {
        &self.children[index]
    }

    #[must_use]
    pub fn parent_index_of(&self, index: usize) -> Option<usize> {
        self.parents[index]
    }

    #[must_use]
    pub fn node_id_at(&self, index: usize) -> NodeId {
        self.node_ids[index]
    }

    #[must_use]
    pub fn index_of(&self, node_id: NodeId) -> Option<usize> {
        self.index_by_id.get(&node_id).copied()
    }

    #[must_use]
    pub fn is_container_like(&self, index: usize) -> bool {
        self.container_like[index]
    }

    #[cfg(test)]
    #[must_use]
    pub fn node_ids(&self) -> &[NodeId] {
        &self.node_ids
    }
}

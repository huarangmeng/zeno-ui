use std::collections::HashMap;

use crate::{
    Axis, ImageNode, InteractionState, Node, NodeId, NodeKind, SpacerNode, Style, TextNode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementId(pub u64);

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FrontendObjectTable {
    pub objects: Vec<FrontendObject>,
    index_by_id: HashMap<NodeId, usize>,
    index_by_element: HashMap<ElementId, usize>,
    node_ids: Vec<NodeId>,
    element_ids: Vec<ElementId>,
    parents: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    container_like: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FrontendObject {
    pub node_id: NodeId,
    pub element_id: ElementId,
    pub kind: FrontendObjectKind,
    pub style: Style,
    pub interaction: InteractionState,
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
    let mut index_by_element = HashMap::new();
    let mut node_ids = Vec::new();
    let mut parents = Vec::new();
    let mut children_table = Vec::new();
    let mut container_like = Vec::new();
    let mut element_ids = Vec::new();

    fn assign_indices(
        node: &Node,
        parent: Option<usize>,
        child_ordinal: usize,
        parent_element_id: Option<ElementId>,
        index_by_id: &mut HashMap<NodeId, usize>,
        index_by_element: &mut HashMap<ElementId, usize>,
        node_ids: &mut Vec<NodeId>,
        parents: &mut Vec<Option<usize>>,
        children_table: &mut Vec<Vec<usize>>,
        container_like: &mut Vec<bool>,
        element_ids: &mut Vec<ElementId>,
    ) -> usize {
        let index = node_ids.len();
        let element_id = ElementId(stable_element_id(
            parent_element_id,
            child_ordinal,
            node_kind_discriminant(&node.kind),
            node.identity_key,
        ));
        index_by_id.insert(node.id(), index);
        index_by_element.insert(element_id, index);
        node_ids.push(node.id());
        parents.push(parent);
        children_table.push(Vec::new());
        let is_container = matches!(
            node.kind,
            NodeKind::Container(_) | NodeKind::Box { .. } | NodeKind::Stack { .. }
        );
        container_like.push(is_container);
        element_ids.push(element_id);

        match &node.kind {
            NodeKind::Container(child) => {
                let child_index = assign_indices(
                    child,
                    Some(index),
                    0,
                    Some(element_id),
                    index_by_id,
                    index_by_element,
                    node_ids,
                    parents,
                    children_table,
                    container_like,
                    element_ids,
                );
                children_table[index].push(child_index);
            }
            NodeKind::Box { children } | NodeKind::Stack { children, .. } => {
                for (child_ordinal, child) in children.iter().enumerate() {
                    let child_index = assign_indices(
                        child,
                        Some(index),
                        child_ordinal,
                        Some(element_id),
                        index_by_id,
                        index_by_element,
                        node_ids,
                        parents,
                        children_table,
                        container_like,
                        element_ids,
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
        0,
        None,
        &mut index_by_id,
        &mut index_by_element,
        &mut node_ids,
        &mut parents,
        &mut children_table,
        &mut container_like,
        &mut element_ids,
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
            element_id: element_ids[index],
            kind: FrontendObjectKind::Box,
            style: Style::default(),
            interaction: InteractionState::default(),
            parent: parents[index],
            first_child,
            next_sibling,
        });
    }
    fill_objects(root, 0, &children_table, &mut objects);

    FrontendObjectTable {
        objects,
        index_by_id,
        index_by_element,
        node_ids,
        element_ids,
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
    objects[index].interaction = node.modifiers.resolve_interaction();

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

fn stable_element_id(
    parent: Option<ElementId>,
    child_ordinal: usize,
    kind_tag: u64,
    explicit_key: Option<u64>,
) -> u64 {
    let mut hash = parent.map_or(0x9e3779b97f4a7c15, |id| id.0);
    hash = mix_hash(hash, kind_tag);
    hash = mix_hash(hash, child_ordinal as u64);
    if let Some(explicit_key) = explicit_key {
        hash = mix_hash(hash, explicit_key);
    }
    hash
}

fn mix_hash(seed: u64, value: u64) -> u64 {
    let mixed = value.wrapping_add(0x9e3779b97f4a7c15);
    let rotated = mixed.rotate_left(27);
    seed ^ rotated.wrapping_mul(0x94d049bb133111eb)
}

fn node_kind_discriminant(kind: &NodeKind) -> u64 {
    match kind {
        NodeKind::Text(_) => 1,
        NodeKind::Image(_) => 2,
        NodeKind::Container(_) => 3,
        NodeKind::Box { .. } => 4,
        NodeKind::Stack { axis, .. } => match axis {
            Axis::Horizontal => 5,
            Axis::Vertical => 6,
        },
        NodeKind::Spacer(_) => 7,
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
    pub fn element_id_at(&self, index: usize) -> ElementId {
        self.element_ids[index]
    }

    #[must_use]
    pub fn index_of(&self, node_id: NodeId) -> Option<usize> {
        self.index_by_id.get(&node_id).copied()
    }

    #[must_use]
    pub fn index_of_element(&self, element_id: ElementId) -> Option<usize> {
        self.index_by_element.get(&element_id).copied()
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

    #[cfg(test)]
    #[must_use]
    pub fn element_ids(&self) -> &[ElementId] {
        &self.element_ids
    }
}

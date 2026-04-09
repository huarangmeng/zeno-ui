//! 这些索引把节点树转换成 retained runtime 需要的快速查询表。

use std::collections::{HashMap, HashSet};

use crate::{
    Node, NodeId, NodeKind,
    layout::{MeasuredKind, MeasuredNode},
};

pub(super) fn index_measured_nodes(
    root: &Node,
    measured: &MeasuredNode,
) -> HashMap<NodeId, MeasuredNode> {
    let mut indexed = HashMap::new();
    collect_measured_nodes(root, measured, &mut indexed);
    indexed
}

pub(super) fn index_parent_nodes(root: &Node) -> HashMap<NodeId, NodeId> {
    let mut indexed = HashMap::new();
    collect_parent_nodes(root, &mut indexed);
    indexed
}

pub(super) fn index_container_like_nodes(root: &Node) -> HashSet<NodeId> {
    let mut indexed = HashSet::new();
    collect_container_like_nodes(root, &mut indexed);
    indexed
}

fn collect_measured_nodes(
    node: &Node,
    measured: &MeasuredNode,
    indexed: &mut HashMap<NodeId, MeasuredNode>,
) {
    indexed.insert(node.id(), measured.clone());

    match (&node.kind, &measured.kind) {
        (NodeKind::Container(child), MeasuredKind::Single(measured_child)) => {
            collect_measured_nodes(child, measured_child, indexed);
        }
        (NodeKind::Stack { children, .. }, MeasuredKind::Multiple(measured_children)) => {
            for (child, measured_child) in children.iter().zip(measured_children.iter()) {
                collect_measured_nodes(child, measured_child, indexed);
            }
        }
        _ => {}
    }
}

fn collect_parent_nodes(node: &Node, indexed: &mut HashMap<NodeId, NodeId>) {
    match &node.kind {
        NodeKind::Container(child) => {
            indexed.insert(child.id(), node.id());
            collect_parent_nodes(child, indexed);
        }
        NodeKind::Stack { children, .. } => {
            for child in children {
                indexed.insert(child.id(), node.id());
                collect_parent_nodes(child, indexed);
            }
        }
        _ => {}
    }
}

fn collect_container_like_nodes(node: &Node, indexed: &mut HashSet<NodeId>) {
    match &node.kind {
        NodeKind::Container(child) => {
            indexed.insert(node.id());
            collect_container_like_nodes(child, indexed);
        }
        NodeKind::Stack { children, .. } => {
            indexed.insert(node.id());
            for child in children {
                collect_container_like_nodes(child, indexed);
            }
        }
        _ => {}
    }
}

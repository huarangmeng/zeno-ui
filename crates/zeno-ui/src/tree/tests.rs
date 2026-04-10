//! retained tree 测试单独拆分，避免核心实现文件继续膨胀。

use std::sync::atomic::{AtomicU64, Ordering};

use zeno_core::{Point, Size};
use zeno_scene::Scene;
use zeno_text::FallbackTextSystem;

use super::RetainedComposeTree;
use crate::render::FragmentStore;
use crate::{DirtyReason, Node, NodeId, NodeKind, TextNode, layout::measure_node};

static NEXT_NODE_ID: AtomicU64 = AtomicU64::new(1);

fn next_node_id() -> NodeId {
    NodeId(NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed))
}

fn text(content: impl Into<String>) -> Node {
    Node::new(
        next_node_id(),
        NodeKind::Text(TextNode {
            content: content.into(),
            font: zeno_text::FontDescriptor::default(),
            font_size: 16.0,
        }),
    )
}

fn column(children: Vec<Node>) -> Node {
    Node::new(
        next_node_id(),
        NodeKind::Stack {
            axis: crate::Axis::Vertical,
            children,
        },
    )
}

fn row(children: Vec<Node>) -> Node {
    Node::new(
        next_node_id(),
        NodeKind::Stack {
            axis: crate::Axis::Horizontal,
            children,
        },
    )
}

#[test]
fn text_dirty_keeps_leaf_as_layout_root() {
    let root = column(vec![text("Title").key("title"), text("Body").key("body")]).key("root");
    let body_id = text("Body").key("body").id();
    let mut retained = retained_tree_for(root, Size::new(320.0, 240.0));

    retained.mark_node_dirty(body_id, DirtyReason::Text);

    assert_eq!(sorted_roots(&retained), vec![body_id]);
}

#[test]
fn structure_dirty_promotes_to_parent_layout_root() {
    let root = column(vec![text("Title").key("title"), text("Body").key("body")]).key("root");
    let root_id = root.id();
    let body_id = text("Body").key("body").id();
    let mut retained = retained_tree_for(root, Size::new(320.0, 240.0));

    retained.mark_node_dirty(body_id, DirtyReason::Structure);

    assert_eq!(sorted_roots(&retained), vec![root_id]);
}

#[test]
fn structure_dirty_on_container_stays_local_to_container_root() {
    let card = column(vec![text("Title").key("title"), text("Body").key("body")]).key("card");
    let card_id = card.id();
    let root = row(vec![card, text("Side").key("side")]).key("root");
    let mut retained = retained_tree_for(root, Size::new(320.0, 240.0));

    retained.mark_node_dirty(card_id, DirtyReason::Structure);

    assert_eq!(sorted_roots(&retained), vec![card_id]);
}

#[test]
fn order_dirty_keeps_stack_node_as_local_root() {
    let stack = column(vec![text("A").key("a"), text("B").key("b")]).key("stack");
    let stack_id = stack.id();
    let root = row(vec![stack, text("Side").key("side")]).key("root");
    let mut retained = retained_tree_for(root, Size::new(320.0, 240.0));

    retained.mark_node_dirty(stack_id, DirtyReason::Order);

    assert_eq!(sorted_roots(&retained), vec![stack_id]);
}

#[test]
fn sibling_dirty_nodes_remain_independent_leaf_roots() {
    let root = column(vec![text("A").key("a"), text("B").key("b")]).key("root");
    let id_a = text("A").key("a").id();
    let id_b = text("B").key("b").id();
    let mut retained = retained_tree_for(root, Size::new(320.0, 240.0));
    retained.mark_node_dirty(id_a, DirtyReason::Layout);
    retained.mark_node_dirty(id_b, DirtyReason::Layout);
    let roots = sorted_roots(&retained);
    assert_eq!(roots, vec![id_a, id_b]);
}

#[test]
fn dirty_nodes_in_different_containers_stay_scoped_to_their_branches() {
    let left = column(vec![text("L1").key("l1"), text("L2").key("l2")]).key("left");
    let right = column(vec![text("R1").key("r1"), text("R2").key("r2")]).key("right");
    let root = row(vec![left, right]).key("root");
    let id_l2 = text("L2").key("l2").id();
    let id_r2 = text("R2").key("r2").id();
    let mut retained = retained_tree_for(root, Size::new(320.0, 240.0));
    retained.mark_node_dirty(id_l2, DirtyReason::Layout);
    retained.mark_node_dirty(id_r2, DirtyReason::Layout);
    let roots = sorted_roots(&retained);
    assert_eq!(roots, vec![id_l2, id_r2]);
}

fn retained_tree_for(root: Node, viewport: Size) -> RetainedComposeTree {
    let measured = measure_node(&root, Point::new(0.0, 0.0), viewport, &FallbackTextSystem);
    let layout = crate::layout::LayoutArena::from_measured(&root, &measured);
    RetainedComposeTree::new(
        root,
        viewport,
        layout,
        Vec::new(),
        FragmentStore::new_with_len(0),
        Scene::new(viewport),
    )
}

fn sorted_roots(retained: &RetainedComposeTree) -> Vec<NodeId> {
    let mut roots: Vec<NodeId> = retained
        .layout_dirty_root_indices()
        .into_iter()
        .map(|index| retained.layout().index_table().node_id_at(index))
        .collect();
    roots.sort_by_key(|id| id.0);
    roots
}

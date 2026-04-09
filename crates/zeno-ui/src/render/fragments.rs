//! 片段与 available map 的构建拆出来，便于后续单独优化缓存命中策略。

use super::scene::build_scene;
use super::*;
use crate::layout::LayoutArena;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandRange {
    pub start: usize,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct FragmentStore {
    commands: Vec<DrawCommand>,
    ranges_by_node: HashMap<NodeId, CommandRange>,
}

impl FragmentStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, node_id: NodeId, fragment: Vec<DrawCommand>) {
        let range = self.append(&fragment);
        self.ranges_by_node.insert(node_id, range);
    }

    #[must_use]
    pub fn fragment_range(&self, node_id: NodeId) -> Option<CommandRange> {
        self.ranges_by_node.get(&node_id).copied()
    }

    #[must_use]
    pub fn fragment(&self, node_id: NodeId) -> Option<&[DrawCommand]> {
        self.fragment_range(node_id)
            .map(|range| &self.commands[range.start..range.start + range.len])
    }

    #[must_use]
    pub fn clone_fragment(&self, node_id: NodeId) -> Option<Vec<DrawCommand>> {
        self.fragment(node_id).map(|fragment| fragment.to_vec())
    }

    pub fn retain(&mut self, valid_ids: &HashSet<NodeId>) {
        self.ranges_by_node.retain(|node_id, _| valid_ids.contains(node_id));
        self.compact();
    }

    #[must_use]
    pub fn active_command_count(&self) -> usize {
        self.ranges_by_node.values().map(|range| range.len).sum()
    }

    fn append(&mut self, fragment: &[DrawCommand]) -> CommandRange {
        let start = self.commands.len();
        self.commands.extend_from_slice(fragment);
        CommandRange {
            start,
            len: fragment.len(),
        }
    }

    fn compact(&mut self) {
        let mut rebuilt = Vec::with_capacity(self.active_command_count());
        let mut rebuilt_ranges = HashMap::with_capacity(self.ranges_by_node.len());
        let mut entries: Vec<_> = self
            .ranges_by_node
            .iter()
            .map(|(node_id, range)| (*node_id, *range))
            .collect();
        entries.sort_by_key(|(node_id, _)| node_id.0);
        for (node_id, range) in entries {
            let start = rebuilt.len();
            rebuilt.extend_from_slice(&self.commands[range.start..range.start + range.len]);
            rebuilt_ranges.insert(
                node_id,
                CommandRange {
                    start,
                    len: range.len,
                },
            );
        }
        self.commands = rebuilt;
        self.ranges_by_node = rebuilt_ranges;
    }
}

pub(super) fn structured_scene_from_layout(
    root: &Node,
    viewport: Size,
    layout: &LayoutArena,
) -> (
    HashMap<NodeId, Size>,
    FragmentStore,
    Scene,
) {
    let mut fragments = FragmentStore::new();
    let mut available_by_node = HashMap::new();
    collect_fragments(
        root,
        layout,
        viewport,
        &mut available_by_node,
        &mut fragments,
    );
    let scene = build_scene(root, layout, viewport, &fragments);
    (available_by_node, fragments, scene)
}

pub(super) fn available_map_from_layout(
    root: &Node,
    viewport: Size,
    layout: &LayoutArena,
) -> HashMap<NodeId, Size> {
    let mut available_by_node = HashMap::new();
    collect_available(root, layout, viewport, &mut available_by_node);
    available_by_node
}

pub(super) fn collect_fragments(
    node: &Node,
    layout: &LayoutArena,
    available: Size,
    available_by_node: &mut HashMap<NodeId, Size>,
    fragments: &mut FragmentStore,
) {
    available_by_node.insert(node.id(), available);
    if let Some(slot) = layout.slot(node.id()) {
        fragments.insert(node.id(), node_fragment(node, slot, layout));
    }

    match &node.kind {
        NodeKind::Container(child) => {
            collect_fragments(
                child,
                layout,
                crate::layout::content_available(node, available),
                available_by_node,
                fragments,
            );
        }
        NodeKind::Box { children } => {
            let child_available = crate::layout::content_available(node, available);
            for child in children {
                collect_fragments(
                    child,
                    layout,
                    child_available,
                    available_by_node,
                    fragments,
                );
            }
        }
        NodeKind::Stack { children, .. } => {
            collect_stack_fragments(
                node,
                children,
                layout,
                available,
                available_by_node,
                fragments,
            );
        }
        _ => {}
    }
}

fn collect_available(
    node: &Node,
    layout: &LayoutArena,
    available: Size,
    available_by_node: &mut HashMap<NodeId, Size>,
) {
    available_by_node.insert(node.id(), available);
    match &node.kind {
        NodeKind::Container(child) => {
            collect_available(
                child,
                layout,
                crate::layout::content_available(node, available),
                available_by_node,
            );
        }
        NodeKind::Box { children } => {
            let child_available = crate::layout::content_available(node, available);
            for child in children {
                collect_available(child, layout, child_available, available_by_node);
            }
        }
        NodeKind::Stack { children, .. } => {
            let content_available = crate::layout::content_available(node, available);
            let axis = child_axis(node);
            let mut used_main = 0.0f32;
            for (index, child) in children.iter().enumerate() {
                let child_available =
                    crate::layout::remaining_available_for_axis(content_available, used_main, axis);
                collect_available(child, layout, child_available, available_by_node);
                let child_frame = layout
                    .frame(child.id())
                    .expect("layout frame should exist for stack child");
                used_main += main_axis_extent(child_frame.size, axis);
                if index + 1 != children.len() {
                    used_main += node.resolved_style().spacing;
                }
            }
        }
        _ => {}
    }
}

pub(super) fn node_fragment(
    node: &Node,
    slot: &crate::layout::LayoutSlot,
    layout: &LayoutArena,
) -> Vec<DrawCommand> {
    let style = node.resolved_style();
    let mut fragment = Vec::new();
    let local_bounds = Rect::new(
        0.0,
        0.0,
        slot.frame.size.width,
        slot.frame.size.height,
    );
    if let Some(background) = style.background {
        let shape = if style.corner_radius > 0.0 {
            Shape::RoundedRect {
                rect: local_bounds,
                radius: style.corner_radius,
            }
        } else {
            Shape::Rect(local_bounds)
        };
        fragment.push(DrawCommand::Fill {
            shape,
            brush: Brush::Solid(background),
        });
    }

    if let (NodeKind::Text(_), Some(text_layout)) = (&node.kind, layout.text_layout(node.id())) {
        let position = Point::new(
            style.padding.left,
            style.padding.top + text_layout.metrics.ascent,
        );
        fragment.push(DrawCommand::Text {
            position,
            layout: text_layout.clone(),
            color: style.foreground,
        });
    }

    fragment
}

pub(super) fn find_node(node: &Node, node_id: NodeId) -> Option<&Node> {
    if node.id() == node_id {
        return Some(node);
    }

    match &node.kind {
        NodeKind::Container(child) => find_node(child, node_id),
        NodeKind::Box { children } | NodeKind::Stack { children, .. } => {
            children.iter().find_map(|child| find_node(child, node_id))
        }
        _ => None,
    }
}

fn collect_stack_fragments(
    node: &Node,
    children: &[Node],
    layout: &LayoutArena,
    available: Size,
    available_by_node: &mut HashMap<NodeId, Size>,
    fragments: &mut FragmentStore,
) {
    let content_available = crate::layout::content_available(node, available);
    let mut used_main = 0.0f32;
    let axis = child_axis(node);
    for (index, child) in children.iter().enumerate() {
        let child_available =
            crate::layout::remaining_available_for_axis(content_available, used_main, axis);
        collect_fragments(
            child,
            layout,
            child_available,
            available_by_node,
            fragments,
        );
        let child_frame = layout
            .frame(child.id())
            .expect("layout frame should exist for stack child");
        used_main += main_axis_extent(child_frame.size, axis);
        if index + 1 != children.len() {
            used_main += node.resolved_style().spacing;
        }
    }
}

pub(super) fn child_axis(node: &Node) -> crate::Axis {
    match &node.kind {
        NodeKind::Stack { axis, .. } => *axis,
        _ => crate::Axis::Vertical,
    }
}

pub(super) fn main_axis_extent(size: Size, axis: crate::Axis) -> f32 {
    match axis {
        crate::Axis::Horizontal => size.width,
        crate::Axis::Vertical => size.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Node, NodeId, NodeKind, SpacerNode};
    use zeno_core::Color;
    use zeno_text::FallbackTextSystem;

    fn next_node_id() -> NodeId {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_NODE_ID: AtomicU64 = AtomicU64::new(1);
        NodeId(NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed))
    }

    fn spacer(width: f32, height: f32) -> Node {
        Node::new(next_node_id(), NodeKind::Spacer(SpacerNode { width, height }))
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
    fn available_map_uses_shared_available_helpers_for_stack_children() {
        let first = spacer(30.0, 10.0);
        let second = spacer(30.0, 10.0);
        let third = spacer(30.0, 10.0);
        let first_id = first.id();
        let second_id = second.id();
        let third_id = third.id();
        let root = row(vec![first, second, third])
            .padding_all(5.0)
            .spacing(7.0);
        let viewport = Size::new(70.0, 40.0);

        let measured =
            crate::layout::measure_node(&root, Point::new(0.0, 0.0), viewport, &FallbackTextSystem);
        let layout = crate::layout::LayoutArena::from_measured(&root, &measured);
        let available_by_node = available_map_from_layout(&root, viewport, &layout);

        assert_eq!(available_by_node[&root.id()], viewport);
        assert_eq!(available_by_node[&first_id], Size::new(60.0, 30.0));
        assert_eq!(available_by_node[&second_id], Size::new(23.0, 30.0));
        assert_eq!(available_by_node[&third_id], Size::new(0.0, 30.0));
    }

    #[test]
    fn fragment_store_uses_ranges_over_shared_buffer() {
        let id_a = next_node_id();
        let id_b = next_node_id();
        let mut store = FragmentStore::new();

        store.insert(id_a, vec![DrawCommand::Clear(Color::WHITE)]);
        store.insert(id_b, vec![
            DrawCommand::Clear(Color::BLACK),
            DrawCommand::Clear(Color::TRANSPARENT),
        ]);

        assert_eq!(store.fragment_range(id_a), Some(CommandRange { start: 0, len: 1 }));
        assert_eq!(store.fragment_range(id_b), Some(CommandRange { start: 1, len: 2 }));
        assert_eq!(store.active_command_count(), 3);
    }
}

//! 片段与 available map 的构建拆出来，便于后续单独优化缓存命中策略。

use super::scene::build_scene;
use super::*;
use crate::layout::LayoutArena;
use crate::tree::NodeIndexTable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandRange {
    pub start: usize,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct FragmentStore {
    commands: Vec<DrawCommand>,
    ranges_by_index: Vec<Option<CommandRange>>,
}

impl FragmentStore {
    #[must_use]
    pub fn new_with_len(len: usize) -> Self {
        Self {
            commands: Vec::new(),
            ranges_by_index: vec![None; len],
        }
    }

    pub fn insert_at(&mut self, index: usize, fragment: Vec<DrawCommand>) {
        let range = self.append(&fragment);
        if index >= self.ranges_by_index.len() {
            self.ranges_by_index.resize(index + 1, None);
        }
        self.ranges_by_index[index] = Some(range);
    }

    #[must_use]
    pub fn fragment_range_at(&self, index: usize) -> Option<CommandRange> {
        self.ranges_by_index.get(index).copied().flatten()
    }

    #[must_use]
    pub fn fragment_at(&self, index: usize) -> Option<&[DrawCommand]> {
        self.fragment_range_at(index)
            .map(|range| &self.commands[range.start..range.start + range.len])
    }

    #[must_use]
    pub fn clone_fragment_at(&self, index: usize) -> Option<Vec<DrawCommand>> {
        self.fragment_at(index).map(|fragment| fragment.to_vec())
    }

    pub fn remap(
        &mut self,
        old_index_table: &NodeIndexTable,
        new_index_table: &NodeIndexTable,
    ) {
        let mut remapped = vec![None; new_index_table.len()];
        for (old_index, maybe_range) in self.ranges_by_index.iter().copied().enumerate() {
            let Some(range) = maybe_range else {
                continue;
            };
            let node_id = old_index_table.node_ids()[old_index];
            if let Some(new_index) = new_index_table.index_of(node_id) {
                remapped[new_index] = Some(range);
            }
        }
        self.ranges_by_index = remapped;
        self.compact();
    }

    #[must_use]
    pub fn active_command_count(&self) -> usize {
        self.ranges_by_index
            .iter()
            .flatten()
            .map(|range| range.len)
            .sum()
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
        let mut rebuilt_ranges = vec![None; self.ranges_by_index.len()];
        for (index, maybe_range) in self.ranges_by_index.iter().copied().enumerate() {
            let Some(range) = maybe_range else {
                continue;
            };
            let start = rebuilt.len();
            rebuilt.extend_from_slice(&self.commands[range.start..range.start + range.len]);
            rebuilt_ranges[index] = Some(CommandRange {
                start,
                len: range.len,
            });
        }
        self.commands = rebuilt;
        self.ranges_by_index = rebuilt_ranges;
    }
}

pub(super) fn structured_scene_from_layout(
    root: &Node,
    viewport: Size,
    layout: &LayoutArena,
) -> (
    Vec<Size>,
    FragmentStore,
    Scene,
) {
    let mut fragments = FragmentStore::new_with_len(layout.index_table().len());
    let mut available = vec![Size::new(0.0, 0.0); layout.index_table().len()];
    collect_fragments(
        root,
        0,
        layout,
        viewport,
        &mut available,
        &mut fragments,
    );
    let scene = build_scene(root, layout, viewport, &fragments);
    (available, fragments, scene)
}

pub(super) fn available_slots_from_layout(
    root: &Node,
    viewport: Size,
    layout: &LayoutArena,
) -> Vec<Size> {
    let mut available = vec![Size::new(0.0, 0.0); layout.index_table().len()];
    collect_available(root, 0, layout, viewport, &mut available);
    available
}

pub(super) fn collect_fragments(
    node: &Node,
    index: usize,
    layout: &LayoutArena,
    available: Size,
    available_by_index: &mut [Size],
    fragments: &mut FragmentStore,
) {
    available_by_index[index] = available;
    if let Some(slot) = layout.slot(node.id()) {
        fragments.insert_at(index, node_fragment(node, slot, layout));
    }

    match &node.kind {
        NodeKind::Container(child) => {
            let child_index = layout.index_table().child_indices(index)[0];
            collect_fragments(
                child,
                child_index,
                layout,
                crate::layout::content_available(node, available),
                available_by_index,
                fragments,
            );
        }
        NodeKind::Box { children } => {
            let child_available = crate::layout::content_available(node, available);
            for (child, child_index) in children
                .iter()
                .zip(layout.index_table().child_indices(index).iter().copied())
            {
                collect_fragments(
                    child,
                    child_index,
                    layout,
                    child_available,
                    available_by_index,
                    fragments,
                );
            }
        }
        NodeKind::Stack { children, .. } => {
            collect_stack_fragments(
                node,
                children,
                index,
                layout,
                available,
                available_by_index,
                fragments,
            );
        }
        _ => {}
    }
}

fn collect_available(
    node: &Node,
    index: usize,
    layout: &LayoutArena,
    available: Size,
    available_by_index: &mut [Size],
) {
    available_by_index[index] = available;
    match &node.kind {
        NodeKind::Container(child) => {
            let child_index = layout.index_table().child_indices(index)[0];
            collect_available(
                child,
                child_index,
                layout,
                crate::layout::content_available(node, available),
                available_by_index,
            );
        }
        NodeKind::Box { children } => {
            let child_available = crate::layout::content_available(node, available);
            for (child, child_index) in children
                .iter()
                .zip(layout.index_table().child_indices(index).iter().copied())
            {
                collect_available(child, child_index, layout, child_available, available_by_index);
            }
        }
        NodeKind::Stack { children, .. } => {
            let content_available = crate::layout::content_available(node, available);
            let axis = child_axis(node);
            let mut used_main = 0.0f32;
            for (position, (child, child_index)) in children
                .iter()
                .zip(layout.index_table().child_indices(index).iter().copied())
                .enumerate()
            {
                let child_available =
                    crate::layout::remaining_available_for_axis(content_available, used_main, axis);
                collect_available(child, child_index, layout, child_available, available_by_index);
                let child_frame = layout
                    .frame(child.id())
                    .expect("layout frame should exist for stack child");
                used_main += main_axis_extent(child_frame.size, axis);
                if position + 1 != children.len() {
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

fn collect_stack_fragments(
    node: &Node,
    children: &[Node],
    index: usize,
    layout: &LayoutArena,
    available: Size,
    available_by_index: &mut [Size],
    fragments: &mut FragmentStore,
) {
    let content_available = crate::layout::content_available(node, available);
    let mut used_main = 0.0f32;
    let axis = child_axis(node);
    for (position, (child, child_index)) in children
        .iter()
        .zip(layout.index_table().child_indices(index).iter().copied())
        .enumerate()
    {
        let child_available =
            crate::layout::remaining_available_for_axis(content_available, used_main, axis);
        collect_fragments(
            child,
            child_index,
            layout,
            child_available,
            available_by_index,
            fragments,
        );
        let child_frame = layout
            .frame(child.id())
            .expect("layout frame should exist for stack child");
        used_main += main_axis_extent(child_frame.size, axis);
        if position + 1 != children.len() {
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
        let available = available_slots_from_layout(&root, viewport, &layout);
        let table = layout.index_table();

        assert_eq!(available[table.index_of(root.id()).unwrap()], viewport);
        assert_eq!(available[table.index_of(first_id).unwrap()], Size::new(60.0, 30.0));
        assert_eq!(available[table.index_of(second_id).unwrap()], Size::new(23.0, 30.0));
        assert_eq!(available[table.index_of(third_id).unwrap()], Size::new(0.0, 30.0));
    }

    #[test]
    fn fragment_store_uses_ranges_over_shared_buffer() {
        let mut store = FragmentStore::new_with_len(2);

        store.insert_at(0, vec![DrawCommand::Clear(Color::WHITE)]);
        store.insert_at(1, vec![
            DrawCommand::Clear(Color::BLACK),
            DrawCommand::Clear(Color::TRANSPARENT),
        ]);

        assert_eq!(store.fragment_range_at(0), Some(CommandRange { start: 0, len: 1 }));
        assert_eq!(store.fragment_range_at(1), Some(CommandRange { start: 1, len: 2 }));
        assert_eq!(store.active_command_count(), 3);
    }
}

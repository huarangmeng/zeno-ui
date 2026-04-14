//! 片段与 available map 的构建拆出来，便于后续单独优化缓存命中策略。

use super::*;
#[cfg(test)]
use crate::frontend::FrontendObjectTable;
use crate::frontend::{FrontendObject, FrontendObjectKind, compile_object_table};
use crate::layout::LayoutArena;
#[cfg(test)]
use zeno_scene::DrawCommand;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommandRange {
    pub start: usize,
    pub len: usize,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct FragmentStore {
    commands: Vec<DrawCommand>,
    ranges_by_index: Vec<Option<CommandRange>>,
}

#[cfg(test)]
#[allow(dead_code)]
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
        old_object_table: &FrontendObjectTable,
        new_object_table: &FrontendObjectTable,
    ) {
        let mut remapped = vec![None; new_object_table.len()];
        for (old_index, maybe_range) in self.ranges_by_index.iter().copied().enumerate() {
            let Some(range) = maybe_range else {
                continue;
            };
            let node_id = old_object_table.node_ids()[old_index];
            if let Some(new_index) = new_object_table.index_of(node_id) {
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

pub(super) fn available_slots_from_layout(
    root: &Node,
    viewport: Size,
    layout: &LayoutArena,
) -> Vec<Size> {
    let objects = compile_object_table(root);
    available_slots_from_objects(&objects, viewport, layout)
}

#[allow(dead_code)]
pub(super) fn child_axis(object: &FrontendObject) -> crate::Axis {
    match object.kind {
        FrontendObjectKind::Stack { axis } => axis,
        _ => crate::Axis::Vertical,
    }
}

pub(super) fn main_axis_extent(size: Size, axis: crate::Axis) -> f32 {
    match axis {
        crate::Axis::Horizontal => size.width,
        crate::Axis::Vertical => size.height,
    }
}

fn available_slots_from_objects(
    objects: &crate::frontend::FrontendObjectTable,
    viewport: Size,
    layout: &LayoutArena,
) -> Vec<Size> {
    let mut available = vec![Size::new(0.0, 0.0); objects.len()];
    if objects.len() == 0 {
        return available;
    }
    available[0] = viewport;
    for index in 0..objects.len() {
        let object = objects.object(index);
        let current = available[index];
        match &object.kind {
            FrontendObjectKind::Container => {
                if let Some(child_index) = object.first_child {
                    available[child_index] = content_available_for_style(&object.style, current);
                }
            }
            FrontendObjectKind::Box => {
                let child_available = content_available_for_style(&object.style, current);
                for &child_index in objects.child_indices(index) {
                    available[child_index] = child_available;
                }
            }
            FrontendObjectKind::Stack { axis } => {
                let content_available = content_available_for_style(&object.style, current);
                let mut used_main = 0.0f32;
                let children = objects.child_indices(index);
                for (position, &child_index) in children.iter().enumerate() {
                    available[child_index] = crate::layout::remaining_available_for_axis(
                        content_available,
                        used_main,
                        *axis,
                    );
                    let child_frame = layout.slot_at(child_index).frame;
                    used_main += main_axis_extent(child_frame.size, *axis);
                    if position + 1 != children.len() {
                        used_main += object.style.spacing;
                    }
                }
            }
            _ => {}
        }
    }
    available
}

fn content_available_for_style(style: &crate::Style, available: Size) -> Size {
    Size::new(
        (available.width - style.padding.horizontal()).max(0.0),
        (available.height - style.padding.vertical()).max(0.0),
    )
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
        Node::new(
            next_node_id(),
            NodeKind::Spacer(SpacerNode { width, height }),
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
        let table = layout.object_table();

        assert_eq!(available[table.index_of(root.id()).unwrap()], viewport);
        assert_eq!(
            available[table.index_of(first_id).unwrap()],
            Size::new(60.0, 30.0)
        );
        assert_eq!(
            available[table.index_of(second_id).unwrap()],
            Size::new(53.0, 30.0)
        );
        assert_eq!(
            available[table.index_of(third_id).unwrap()],
            Size::new(46.0, 30.0)
        );
        assert!(
            available[table.index_of(first_id).unwrap()].width
                > available[table.index_of(second_id).unwrap()].width
        );
        assert!(
            available[table.index_of(second_id).unwrap()].width
                > available[table.index_of(third_id).unwrap()].width
        );
    }

    #[test]
    fn fragment_store_uses_ranges_over_shared_buffer() {
        let mut store = FragmentStore::new_with_len(2);

        store.insert_at(0, vec![DrawCommand::Clear(Color::WHITE)]);
        store.insert_at(
            1,
            vec![
                DrawCommand::Clear(Color::BLACK),
                DrawCommand::Clear(Color::TRANSPARENT),
            ],
        );

        assert_eq!(
            store.fragment_range_at(0),
            Some(CommandRange { start: 0, len: 1 })
        );
        assert_eq!(
            store.fragment_range_at(1),
            Some(CommandRange { start: 1, len: 2 })
        );
        assert_eq!(store.active_command_count(), 3);
    }
}

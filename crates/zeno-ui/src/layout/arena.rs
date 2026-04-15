use std::sync::Arc;

use zeno_core::{Point, Rect, Size};
use zeno_text::{TextLayout, TextSystem};

use crate::frontend::FrontendObjectTable;
#[cfg(test)]
use crate::frontend::compile_object_table;
use crate::{Node, NodeId, NodeKind};

use super::work_queue::measure_layout_workqueue;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutSlot {
    pub(crate) frame: Rect,
    pub(crate) text_layout: Option<TextLayout>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LayoutArena {
    object_table: Arc<FrontendObjectTable>,
    slots: Vec<LayoutSlot>,
}

impl LayoutArena {
    #[cfg(test)]
    #[must_use]
    pub fn from_measured(root: &Node, _measured: &MeasuredNode) -> Self {
        let table = Arc::new(compile_object_table(root));
        let mut arena = Self::new(table);
        arena.collect(root, 0);
        arena
    }

    #[must_use]
    pub fn slot(&self, node_id: NodeId) -> Option<&LayoutSlot> {
        self.object_table
            .index_of(node_id)
            .map(|index| &self.slots[index])
    }

    #[must_use]
    pub fn slot_at(&self, index: usize) -> &LayoutSlot {
        &self.slots[index]
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn frame(&self, node_id: NodeId) -> Option<Rect> {
        self.slot(node_id).map(|slot| slot.frame)
    }

    #[must_use]
    #[allow(dead_code)]
    pub fn text_layout(&self, node_id: NodeId) -> Option<&TextLayout> {
        self.slot(node_id)
            .and_then(|slot| slot.text_layout.as_ref())
    }

    #[must_use]
    pub fn object_table(&self) -> &Arc<FrontendObjectTable> {
        &self.object_table
    }

    #[must_use]
    pub fn remap(&self, new_object_table: Arc<FrontendObjectTable>) -> Self {
        let mut remapped = Self::new(new_object_table.clone());
        for (old_index, slot) in self.slots.iter().cloned().enumerate() {
            let element_id = self.object_table.element_id_at(old_index);
            if let Some(new_index) = new_object_table.index_of_element(element_id) {
                remapped.slots[new_index] = slot;
            }
        }
        remapped
    }

    pub(crate) fn new(object_table: Arc<FrontendObjectTable>) -> Self {
        Self {
            slots: vec![
                LayoutSlot {
                    frame: Rect::new(0.0, 0.0, 0.0, 0.0),
                    text_layout: None,
                };
                object_table.len()
            ],
            object_table,
        }
    }

    pub(crate) fn upsert(&mut self, index: usize, frame: Rect, text_layout: Option<TextLayout>) {
        self.slots[index] = LayoutSlot { frame, text_layout };
    }

    pub(crate) fn shift(&mut self, index: usize, dx: f32, dy: f32) {
        let slot = &mut self.slots[index];
        slot.frame = Rect::new(
            slot.frame.origin.x + dx,
            slot.frame.origin.y + dy,
            slot.frame.size.width,
            slot.frame.size.height,
        );
    }

    #[cfg(test)]
    fn collect(&mut self, node: &Node, index: usize) {
        self.slots[index] = LayoutSlot {
            frame: Rect::new(0.0, 0.0, 0.0, 0.0),
            text_layout: None,
        };
        match &node.kind {
            NodeKind::Container(child) => {
                let child_index = self.object_table.child_indices(index)[0];
                self.collect(child, child_index);
            }
            NodeKind::Box { children } | NodeKind::Stack { children, .. } => {
                let child_indices = self.object_table.child_indices(index).to_vec();
                for (child, child_index) in children.iter().zip(child_indices.into_iter()) {
                    self.collect(child, child_index);
                }
            }
            _ => {}
        }
    }
}

#[must_use]
pub(crate) fn measure_layout(
    root: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
) -> LayoutArena {
    measure_layout_workqueue(root, origin, available, text_system)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MeasuredNode {
    pub(crate) frame: Rect,
    pub(crate) kind: MeasuredKind,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MeasuredKind {
    Text(TextLayout),
    Image,
    Single(Box<MeasuredNode>),
    Multiple(Vec<MeasuredNode>),
    Spacer,
}

pub(crate) fn measure_node(
    node: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
) -> MeasuredNode {
    let arena = measure_layout(node, origin, available, text_system);
    measured_from_layout(node, &arena)
}

fn measured_from_layout(node: &Node, arena: &LayoutArena) -> MeasuredNode {
    measured_from_layout_at(node, 0, arena)
}

fn measured_from_layout_at(node: &Node, index: usize, arena: &LayoutArena) -> MeasuredNode {
    let slot = arena.slot_at(index);
    let kind = match &node.kind {
        NodeKind::Text(_) => MeasuredKind::Text(
            slot.text_layout
                .clone()
                .expect("text layout should exist for text node"),
        ),
        NodeKind::Image(_) => MeasuredKind::Image,
        NodeKind::Container(child) => MeasuredKind::Single(Box::new(measured_from_layout_at(
            child,
            arena.object_table.child_indices(index)[0],
            arena,
        ))),
        NodeKind::Box { children } | NodeKind::Stack { children, .. } => MeasuredKind::Multiple(
            children
                .iter()
                .zip(arena.object_table.child_indices(index).iter().copied())
                .map(|(child, child_index)| measured_from_layout_at(child, child_index, arena))
                .collect(),
        ),
        NodeKind::Spacer(_) => MeasuredKind::Spacer,
    };
    MeasuredNode {
        frame: slot.frame,
        kind,
    }
}

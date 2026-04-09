//! 这些索引把节点树转换成 retained runtime 需要的快速查询表。

use std::collections::HashMap;

use zeno_core::Size;

use crate::{
    DirtyFlags, DirtyReason,
    Node, NodeId, NodeKind,
    layout::LayoutArena,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DenseNodeStore {
    index_by_id: HashMap<NodeId, usize>,
    node_ids: Vec<NodeId>,
    parents: Vec<Option<usize>>,
    layout: Vec<crate::layout::LayoutSlot>,
    available: Vec<Size>,
    container_like: Vec<bool>,
    dirty: Vec<DirtyFlags>,
}

impl DenseNodeStore {
    #[must_use]
    pub fn build(
        root: &Node,
        layout: &LayoutArena,
        available_by_node: &HashMap<NodeId, Size>,
    ) -> Self {
        let mut store = Self {
            index_by_id: HashMap::new(),
            node_ids: Vec::new(),
            parents: Vec::new(),
            layout: Vec::new(),
            available: Vec::new(),
            container_like: Vec::new(),
            dirty: Vec::new(),
        };
        store.collect(root, layout, available_by_node, None);
        store
    }

    #[must_use]
    pub fn layout_for(&self, node_id: NodeId) -> Option<&crate::layout::LayoutSlot> {
        self.index_of(node_id).map(|index| &self.layout[index])
    }

    #[must_use]
    pub fn available_for(&self, node_id: NodeId) -> Option<Size> {
        self.index_of(node_id).map(|index| self.available[index])
    }

    #[must_use]
    pub fn dirty_flags_for(&self, node_id: NodeId) -> DirtyFlags {
        self.index_of(node_id)
            .map(|index| self.dirty[index])
            .unwrap_or_else(DirtyFlags::clean)
    }

    pub fn mark_dirty(&mut self, node_id: NodeId, reason: DirtyReason) {
        if let Some(index) = self.index_of(node_id) {
            self.dirty[index].mark(reason);
        }
    }

    pub fn clear_dirty(&mut self, node_id: NodeId) {
        if let Some(index) = self.index_of(node_id) {
            self.dirty[index] = DirtyFlags::clean();
        }
    }

    pub fn clear_all_dirty(&mut self) {
        for flags in &mut self.dirty {
            *flags = DirtyFlags::clean();
        }
    }

    #[must_use]
    pub fn dirty_node_ids(&self) -> Vec<NodeId> {
        self.node_ids
            .iter()
            .zip(self.dirty.iter())
            .filter_map(|(node_id, flags)| (!flags.is_clean()).then_some(*node_id))
            .collect()
    }

    #[must_use]
    pub fn parent_of(&self, node_id: NodeId) -> Option<NodeId> {
        let index = self.index_of(node_id)?;
        self.parents[index].map(|parent| self.node_ids[parent])
    }

    #[must_use]
    pub fn is_container_like(&self, node_id: NodeId) -> bool {
        self.index_of(node_id)
            .map(|index| self.container_like[index])
            .unwrap_or(false)
    }

    #[must_use]
    pub fn node_ids(&self) -> &[NodeId] {
        &self.node_ids
    }

    fn index_of(&self, node_id: NodeId) -> Option<usize> {
        self.index_by_id.get(&node_id).copied()
    }

    fn collect(
        &mut self,
        node: &Node,
        layout: &LayoutArena,
        available_by_node: &HashMap<NodeId, Size>,
        parent: Option<usize>,
    ) {
        let slot = layout
            .slot(node.id())
            .cloned()
            .expect("layout slot should exist for node");
        let index = self.node_ids.len();
        self.index_by_id.insert(node.id(), index);
        self.node_ids.push(node.id());
        self.parents.push(parent);
        self.layout.push(slot);
        self.available
            .push(
                available_by_node
                    .get(&node.id())
                    .copied()
                    .unwrap_or(Size::new(0.0, 0.0)),
            );
        self.container_like.push(matches!(
            node.kind,
            NodeKind::Container(_) | NodeKind::Box { .. } | NodeKind::Stack { .. }
        ));
        self.dirty.push(DirtyFlags::clean());

        match &node.kind {
            NodeKind::Container(child) => {
                self.collect(child, layout, available_by_node, Some(index));
            }
            NodeKind::Box { children } | NodeKind::Stack { children, .. } => {
                for child in children {
                    self.collect(child, layout, available_by_node, Some(index));
                }
            }
            _ => {}
        }
    }
}

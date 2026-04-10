//! 这些索引把节点树转换成 retained runtime 需要的快速查询表。

use std::collections::HashMap;
use std::sync::Arc;

use zeno_core::Size;

use crate::{
    DirtyFlags, DirtyReason,
    Node, NodeId, NodeKind,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NodeIndexTable {
    index_by_id: HashMap<NodeId, usize>,
    node_ids: Vec<NodeId>,
    parents: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    container_like: Vec<bool>,
}

impl NodeIndexTable {
    #[must_use]
    pub fn build(root: &Node) -> Arc<Self> {
        let mut table = Self {
            index_by_id: HashMap::new(),
            node_ids: Vec::new(),
            parents: Vec::new(),
            children: Vec::new(),
            container_like: Vec::new(),
        };
        table.collect(root, None);
        Arc::new(table)
    }

    #[must_use]
    pub fn index_of(&self, node_id: NodeId) -> Option<usize> {
        self.index_by_id.get(&node_id).copied()
    }

    #[must_use]
    pub fn node_id_at(&self, index: usize) -> NodeId {
        self.node_ids[index]
    }

    #[must_use]
    pub fn parent_index_of(&self, index: usize) -> Option<usize> {
        self.parents[index]
    }

    #[must_use]
    pub fn child_indices(&self, index: usize) -> &[usize] {
        &self.children[index]
    }

    #[must_use]
    pub fn is_container_like_index(&self, index: usize) -> bool {
        self.container_like[index]
    }

    #[must_use]
    pub fn node_ids(&self) -> &[NodeId] {
        &self.node_ids
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.node_ids.len()
    }

    fn collect(&mut self, node: &Node, parent: Option<usize>) -> usize {
        let index = self.node_ids.len();
        self.index_by_id.insert(node.id(), index);
        self.node_ids.push(node.id());
        self.parents.push(parent);
        self.children.push(Vec::new());
        self.container_like.push(matches!(
            node.kind,
            NodeKind::Container(_) | NodeKind::Box { .. } | NodeKind::Stack { .. }
        ));

        match &node.kind {
            NodeKind::Container(child) => {
                let child_index = self.collect(child, Some(index));
                self.children[index].push(child_index);
            }
            NodeKind::Box { children } | NodeKind::Stack { children, .. } => {
                for child in children {
                    let child_index = self.collect(child, Some(index));
                    self.children[index].push(child_index);
                }
            }
            _ => {}
        }
        index
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DenseNodeStore {
    index_table: Arc<NodeIndexTable>,
    available: Vec<Size>,
    dirty: Vec<DirtyFlags>,
}

impl DenseNodeStore {
    #[must_use]
    pub fn build(index_table: Arc<NodeIndexTable>, available: Vec<Size>) -> Self {
        let dirty = vec![DirtyFlags::clean(); index_table.len()];
        Self {
            index_table,
            available,
            dirty,
        }
    }

    pub fn mark_dirty_at(&mut self, index: usize, reason: DirtyReason) {
        self.dirty[index].mark(reason);
    }

    pub fn clear_dirty_at(&mut self, index: usize) {
        self.dirty[index] = DirtyFlags::clean();
    }

    pub fn clear_all_dirty(&mut self) {
        for flags in &mut self.dirty {
            *flags = DirtyFlags::clean();
        }
    }

    #[must_use]
    pub fn dirty_indices(&self) -> Vec<usize> {
        self.dirty
            .iter()
            .enumerate()
            .filter_map(|(index, flags)| (!flags.is_clean()).then_some(index))
            .collect()
    }

    #[must_use]
    pub fn parent_index_of(&self, index: usize) -> Option<usize> {
        self.index_table.parent_index_of(index)
    }

    #[must_use]
    pub fn is_container_like_index(&self, index: usize) -> bool {
        self.index_table.is_container_like_index(index)
    }

    #[must_use]
    pub fn node_ids(&self) -> &[NodeId] {
        self.index_table.node_ids()
    }

    #[must_use]
    pub fn node_id_at(&self, index: usize) -> NodeId {
        self.index_table.node_id_at(index)
    }

    #[must_use]
    pub fn index_of(&self, node_id: NodeId) -> Option<usize> {
        self.index_table.index_of(node_id)
    }
}

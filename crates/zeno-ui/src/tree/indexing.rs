//! 这些索引把节点树转换成 retained runtime 需要的快速查询表。

use std::sync::Arc;

use zeno_core::Size;

use crate::{
    DirtyFlags,
    NodeId,
};
use crate::frontend::FrontendObjectTable;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DenseNodeStore {
    object_table: Arc<FrontendObjectTable>,
    available: Vec<Size>,
    dirty: Vec<DirtyFlags>,
}

impl DenseNodeStore {
    #[must_use]
    pub fn build(object_table: Arc<FrontendObjectTable>, available: Vec<Size>) -> Self {
        let dirty = vec![DirtyFlags::clean(); object_table.len()];
        Self {
            object_table,
            available,
            dirty,
        }
    }

    #[must_use]
    pub fn parent_index_of(&self, index: usize) -> Option<usize> {
        self.object_table.parent_index_of(index)
    }

    #[must_use]
    pub fn is_container_like_index(&self, index: usize) -> bool {
        self.object_table.is_container_like(index)
    }

    #[must_use]
    pub fn node_ids(&self) -> &[NodeId] {
        self.object_table.node_ids()
    }

    #[must_use]
    pub fn index_of(&self, node_id: NodeId) -> Option<usize> {
        self.object_table.index_of(node_id)
    }

    #[must_use]
    pub fn available_at(&self, index: usize) -> Size {
        self.available[index]
    }
}

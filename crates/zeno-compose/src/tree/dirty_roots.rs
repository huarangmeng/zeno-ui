//! dirty root 相关逻辑单独拆分，方便持续演进最小脏根策略。

use std::collections::HashSet;

use crate::NodeId;

use super::RetainedComposeTree;

impl RetainedComposeTree {
    pub fn has_descendant_in(&self, ancestor: NodeId, set: &HashSet<NodeId>) -> bool {
        for candidate in set {
            if *candidate != ancestor && self.is_ancestor_or_same(ancestor, *candidate) {
                return true;
            }
        }
        false
    }

    pub(super) fn layout_root_for(&self, node_id: NodeId) -> NodeId {
        self.parent_by_node
            .get(&node_id)
            .copied()
            .unwrap_or(node_id)
    }

    pub(super) fn structure_root_for(&self, node_id: NodeId) -> NodeId {
        if self.container_like_nodes.contains(&node_id) {
            node_id
        } else {
            self.layout_root_for(node_id)
        }
    }

    pub(super) fn insert_layout_dirty_root(&mut self, candidate: NodeId) {
        if self
            .layout_dirty_roots
            .iter()
            .any(|existing| self.is_ancestor_or_same(*existing, candidate))
        {
            return;
        }
        let to_remove: Vec<NodeId> = self
            .layout_dirty_roots
            .iter()
            .copied()
            .filter(|existing| self.is_ancestor_or_same(candidate, *existing))
            .collect();
        for existing in to_remove {
            self.layout_dirty_roots.remove(&existing);
        }
        self.layout_dirty_roots.insert(candidate);
    }

    fn is_ancestor_or_same(&self, ancestor: NodeId, mut node_id: NodeId) -> bool {
        if ancestor == node_id {
            return true;
        }
        while let Some(parent) = self.parent_by_node.get(&node_id).copied() {
            if parent == ancestor {
                return true;
            }
            node_id = parent;
        }
        false
    }
}

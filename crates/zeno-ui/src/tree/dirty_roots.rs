//! dirty root 相关逻辑单独拆分，方便持续演进最小脏根策略。

use crate::NodeId;

use super::RetainedComposeTree;

impl RetainedComposeTree {
    pub fn has_descendant_in(&self, ancestor: NodeId, set: &[NodeId]) -> bool {
        for candidate in set {
            if *candidate != ancestor && self.is_ancestor_or_same(ancestor, *candidate) {
                return true;
            }
        }
        false
    }

    pub(super) fn layout_root_for(&self, node_id: NodeId) -> NodeId {
        self.dense_nodes.parent_of(node_id).unwrap_or(node_id)
    }

    pub(super) fn structure_root_for(&self, node_id: NodeId) -> NodeId {
        if self.dense_nodes.is_container_like(node_id) {
            node_id
        } else {
            self.layout_root_for(node_id)
        }
    }

    pub(super) fn insert_layout_dirty_root(&mut self, candidate: NodeId, merge_siblings: bool) {
        let mut merged_candidate = candidate;
        if merge_siblings {
            loop {
                let mut updated = false;
                for existing in self.layout_dirty_roots.clone() {
                    if existing != merged_candidate
                        && self.is_ancestor_or_same(existing, merged_candidate)
                    {
                        merged_candidate = existing;
                        updated = true;
                        break;
                    }
                    if self.should_merge_layout_roots(existing, merged_candidate) {
                        merged_candidate = self.merge_layout_roots(existing, merged_candidate);
                        updated = true;
                        break;
                    }
                }
                if !updated {
                    break;
                }
            }
        }
        let to_remove: Vec<NodeId> = self
            .layout_dirty_roots
            .iter()
            .copied()
            .filter(|existing| self.is_ancestor_or_same(merged_candidate, *existing))
            .collect();
        for existing in to_remove {
            self.layout_dirty_roots.retain(|root| *root != existing);
        }
        if !self.layout_dirty_roots.contains(&merged_candidate) {
            self.layout_dirty_roots.push(merged_candidate);
        }
    }

    fn should_merge_layout_roots(&self, a: NodeId, b: NodeId) -> bool {
        matches!(
            self.parent_of(a).zip(self.parent_of(b)),
            Some((parent_a, parent_b)) if parent_a == parent_b
        )
    }

    fn merge_layout_roots(&self, a: NodeId, b: NodeId) -> NodeId {
        self.parent_of(a)
            .filter(|parent| self.parent_of(b) == Some(*parent))
            .unwrap_or_else(|| self.structure_root_for(a))
    }

    fn parent_of(&self, node_id: NodeId) -> Option<NodeId> {
        self.dense_nodes.parent_of(node_id)
    }

    fn is_ancestor_or_same(&self, ancestor: NodeId, mut node_id: NodeId) -> bool {
        if ancestor == node_id {
            return true;
        }
        while let Some(parent) = self.dense_nodes.parent_of(node_id) {
            if parent == ancestor {
                return true;
            }
            node_id = parent;
        }
        false
    }
}

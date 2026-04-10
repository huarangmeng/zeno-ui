//! dirty root 相关逻辑单独拆分，方便持续演进最小脏根策略。

use super::RetainedComposeTree;

impl RetainedComposeTree {
    pub(super) fn layout_root_index_for(&self, index: usize) -> usize {
        self.dense_nodes.parent_index_of(index).unwrap_or(index)
    }

    pub(super) fn structure_root_index_for(&self, index: usize) -> usize {
        if self.dense_nodes.is_container_like_index(index) {
            index
        } else {
            self.layout_root_index_for(index)
        }
    }
    pub(super) fn insert_layout_dirty_root(&mut self, candidate_index: usize, merge_siblings: bool) {
        let mut merged_candidate = candidate_index;
        if merge_siblings {
            loop {
                let mut updated = false;
                for existing in self.layout_dirty_roots.clone() {
                    if existing != merged_candidate
                        && self.is_ancestor_or_same_index(existing, merged_candidate)
                    {
                        merged_candidate = existing;
                        updated = true;
                        break;
                    }
                    if self.should_merge_layout_roots_index(existing, merged_candidate) {
                        merged_candidate = self.merge_layout_roots_index(existing, merged_candidate);
                        updated = true;
                        break;
                    }
                }
                if !updated {
                    break;
                }
            }
        }
        let to_remove: Vec<usize> = self
            .layout_dirty_roots
            .iter()
            .copied()
            .filter(|existing| self.is_ancestor_or_same_index(merged_candidate, *existing))
            .collect();
        for existing in to_remove {
            self.layout_dirty_roots.retain(|root| *root != existing);
        }
        if !self.layout_dirty_roots.contains(&merged_candidate) {
            self.layout_dirty_roots.push(merged_candidate);
        }
    }

    fn should_merge_layout_roots_index(&self, a: usize, b: usize) -> bool {
        self.dense_nodes
            .parent_index_of(a)
            .zip(self.dense_nodes.parent_index_of(b))
            .map(|(pa, pb)| pa == pb)
            .unwrap_or(false)
    }

    fn merge_layout_roots_index(&self, a: usize, b: usize) -> usize {
        if self
            .dense_nodes
            .parent_index_of(a)
            .zip(self.dense_nodes.parent_index_of(b))
            .map(|(pa, pb)| pa == pb)
            .unwrap_or(false)
        {
            self.dense_nodes.parent_index_of(a).unwrap_or(a)
        } else {
            self.structure_root_index_for(a)
        }
    }

    fn is_ancestor_or_same_index(&self, ancestor: usize, mut index: usize) -> bool {
        if ancestor == index {
            return true;
        }
        while let Some(parent) = self.dense_nodes.parent_index_of(index) {
            if parent == ancestor {
                return true;
            }
            index = parent;
        }
        false
    }
}

//! RetainedComposeTree 持有增量合成所需的全部缓存快照。

use zeno_core::Size;
use zeno_scene::RetainedDisplayList;

use crate::frontend::{DirtyBits, DirtyTable, FrontendObjectTable, compile_object_table};
use crate::image::ImageResourceTable;
use crate::{DirtyFlags, DirtyReason, Node, NodeId, layout::LayoutArena};

use super::indexing::DenseNodeStore;

#[derive(Debug, Clone)]
pub struct RetainedComposeTree {
    pub(super) root: Node,
    pub(super) objects: FrontendObjectTable,
    pub(super) viewport: Size,
    pub(super) layout: LayoutArena,
    pub(super) dense_nodes: DenseNodeStore,
    pub(super) dirty_table: DirtyTable,
    pub(super) layout_dirty_roots: Vec<usize>,
    pub(super) display_list: RetainedDisplayList,
    pub(super) image_resources: ImageResourceTable,
    pub(super) dirty: DirtyFlags,
}

impl RetainedComposeTree {
    #[must_use]
    pub fn new(
        root: Node,
        viewport: Size,
        layout: LayoutArena,
        available: Vec<Size>,
        image_resources: ImageResourceTable,
        display_list: RetainedDisplayList,
    ) -> Self {
        let dense_nodes = DenseNodeStore::build(layout.object_table().clone(), available);
        let objects = compile_object_table(&root);
        let dirty_table = DirtyTable::new(layout.object_table().len());
        Self {
            root,
            objects,
            viewport,
            layout,
            dense_nodes,
            dirty_table,
            layout_dirty_roots: Vec::new(),
            display_list,
            image_resources,
            dirty: DirtyFlags::clean(),
        }
    }

    #[must_use]
    pub fn display_list(&self) -> &RetainedDisplayList {
        &self.display_list
    }

    #[must_use]
    pub fn root(&self) -> &Node {
        &self.root
    }

    #[must_use]
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    #[must_use]
    pub const fn dirty(&self) -> DirtyFlags {
        self.dirty
    }

    pub fn replace(
        &mut self,
        root: Node,
        viewport: Size,
        layout: LayoutArena,
        available: Vec<Size>,
        image_resources: ImageResourceTable,
        display_list: RetainedDisplayList,
    ) {
        let dense_nodes = DenseNodeStore::build(layout.object_table().clone(), available);
        let objects = compile_object_table(&root);
        let dirty_table = DirtyTable::new(layout.object_table().len());
        let mut display_list = display_list;
        display_list.generation = self.display_list.generation.saturating_add(1);
        self.root = root;
        self.objects = objects;
        self.viewport = viewport;
        self.layout = layout;
        self.dense_nodes = dense_nodes;
        self.dirty_table = dirty_table;
        self.layout_dirty_roots.clear();
        self.display_list = display_list;
        self.image_resources = image_resources;
        self.dirty = DirtyFlags::clean();
    }

    pub fn mark_dirty(&mut self, reason: DirtyReason) {
        self.dirty.mark(reason);
        let root_index = self
            .dense_nodes
            .index_of(self.root.id())
            .expect("root index should exist");
        self.mark_dirty_bits_at(root_index, dirty_bits_for_reason(reason));
        if reason != DirtyReason::Paint {
            self.layout_dirty_roots.clear();
            self.layout_dirty_roots.push(root_index);
        }
    }

    pub fn mark_node_dirty(&mut self, node_id: NodeId, reason: DirtyReason) {
        self.dirty.mark(reason);
        let Some(node_index) = self.dense_nodes.index_of(node_id) else {
            return;
        };
        if reason == DirtyReason::Paint {
            self.mark_dirty_bits_at(node_index, dirty_bits_for_reason(reason));
            return;
        }

        let mut current = Some(node_index);
        while let Some(index) = current {
            self.mark_dirty_bits_at(index, dirty_bits_for_reason(reason));
            current = self.dense_nodes.parent_index_of(index);
        }
        let candidate_index = match reason {
            DirtyReason::Layout | DirtyReason::Text => node_index,
            DirtyReason::Order => {
                if self.dense_nodes.is_container_like_index(node_index) {
                    node_index
                } else {
                    self.layout_root_index_for(node_index)
                }
            }
            DirtyReason::Structure => self.structure_root_index_for(node_index),
            DirtyReason::Paint => node_index,
        };
        self.insert_layout_dirty_root(
            candidate_index,
            matches!(reason, DirtyReason::Order | DirtyReason::Structure),
        );
    }

    #[must_use]
    pub fn dirty_indices(&self) -> Vec<usize> {
        self.dirty_table.dirty_indices().collect()
    }

    #[must_use]
    pub fn layout_dirty_root_indices(&self) -> Vec<usize> {
        if self.layout_dirty_roots.is_empty() && self.dirty.requires_layout() {
            self.dense_nodes
                .index_of(self.root.id())
                .into_iter()
                .collect()
        } else {
            self.layout_dirty_roots.clone()
        }
    }

    #[must_use]
    pub fn objects(&self) -> &FrontendObjectTable {
        &self.objects
    }

    #[must_use]
    pub fn layout(&self) -> &LayoutArena {
        &self.layout
    }

    #[must_use]
    pub fn available_at(&self, index: usize) -> Size {
        self.dense_nodes.available_at(index)
    }

    #[must_use]
    pub fn parent_index_of(&self, index: usize) -> Option<usize> {
        self.dense_nodes.parent_index_of(index)
    }

    pub fn replace_display_list(&mut self, display_list: RetainedDisplayList) {
        let mut display_list = display_list;
        display_list.generation = self.display_list.generation.saturating_add(1);
        self.display_list = display_list;
    }

    pub fn replace_image_resources(&mut self, image_resources: ImageResourceTable) {
        self.image_resources = image_resources;
    }

    pub fn apply_layout_state(
        &mut self,
        root: Node,
        viewport: Size,
        layout: LayoutArena,
        available: Vec<Size>,
    ) {
        let new_object_table = layout.object_table().clone();
        let dense_nodes = DenseNodeStore::build(layout.object_table().clone(), available);
        let objects = compile_object_table(&root);
        self.root = root;
        self.objects = objects;
        self.viewport = viewport;
        self.layout = layout;
        self.dense_nodes = dense_nodes;
        self.dirty_table = DirtyTable::new(new_object_table.len());
        self.layout_dirty_roots.clear();
        self.dirty = DirtyFlags::clean();
    }

    pub fn sync_root(&mut self, root: Node) {
        self.objects = compile_object_table(&root);
        self.root = root;
    }

    fn mark_dirty_bits_at(&mut self, index: usize, bits: DirtyBits) {
        self.dirty_table.mark(index, bits);
        self.dirty_table.bump_generation();
    }
}

fn dirty_bits_for_reason(reason: DirtyReason) -> DirtyBits {
    match reason {
        DirtyReason::Structure => {
            DirtyBits::STYLE
                | DirtyBits::INTRINSIC
                | DirtyBits::LAYOUT
                | DirtyBits::PAINT
                | DirtyBits::SCENE
        }
        DirtyReason::Layout | DirtyReason::Order | DirtyReason::Text => {
            DirtyBits::INTRINSIC | DirtyBits::LAYOUT | DirtyBits::PAINT | DirtyBits::SCENE
        }
        DirtyReason::Paint => DirtyBits::PAINT | DirtyBits::SCENE | DirtyBits::RESOURCE,
    }
}

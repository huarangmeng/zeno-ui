//! render 模块按职责拆分，避免单文件继续膨胀。

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use zeno_core::{Point, Rect, Size};
use zeno_scene::DisplayList;
use zeno_text::TextSystem;

use crate::{
    Node, NodeId, NodeKind, image::ImageResourceTable, invalidation::DirtyReason,
    layout::measure_layout, modifier::BlendMode, tree::RetainedComposeTree,
};

mod debug;
mod display_list;
mod fragments;
mod reconcile;
mod relayout;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ComposeStats {
    pub compose_passes: usize,
    pub layout_passes: usize,
    pub cache_hits: usize,
}

pub struct ComposeRenderer<'a> {
    text_system: &'a dyn TextSystem,
}

impl<'a> ComposeRenderer<'a> {
    #[must_use]
    pub fn new(text_system: &'a dyn TextSystem) -> Self {
        Self { text_system }
    }

    #[must_use]
    pub fn compose(&self, root: &Node, viewport: Size) -> DisplayList {
        self.compose_display_list(root, viewport)
    }

    #[must_use]
    pub fn compose_display_list(&self, root: &Node, viewport: Size) -> DisplayList {
        let layout = measure_layout(root, Point::new(0.0, 0.0), viewport, self.text_system);
        let image_resources = ImageResourceTable::from_frontend(layout.object_table().as_ref());
        let retained =
            display_list::build_retained_display_list(root, &layout, &image_resources, viewport);
        display_list::snapshot_display_list(&retained, viewport)
    }
}

pub struct ComposeEngine<'a> {
    text_system: &'a dyn TextSystem,
    retained: Option<RetainedComposeTree>,
    stats: ComposeStats,
}

pub enum ComposeUpdate {
    Full {
        display_list: DisplayList,
        compose_stats: ComposeStats,
    },
    Delta {
        dirty_bounds: Option<Rect>,
        patch_upserts: usize,
        patch_removes: usize,
        display_list: DisplayList,
        compose_stats: ComposeStats,
    },
}

impl<'a> ComposeEngine<'a> {
    #[must_use]
    pub fn new(text_system: &'a dyn TextSystem) -> Self {
        Self {
            text_system,
            retained: None,
            stats: ComposeStats::default(),
        }
    }

    #[must_use]
    pub fn compose(&mut self, root: &Node, viewport: Size) -> DisplayList {
        self.compose_display_list(root, viewport)
    }

    #[must_use]
    pub fn compose_display_list(&mut self, root: &Node, viewport: Size) -> DisplayList {
        let _ = self.compose_update(root, viewport);
        self.current_display_list()
            .expect("display list should exist after compose")
            .snapshot(viewport)
    }

    pub fn compose_update(&mut self, root: &Node, viewport: Size) -> ComposeUpdate {
        if let Some(retained) = self.retained.as_mut() {
            if retained.viewport() == viewport && retained.root() != root {
                reconcile::reconcile_root_change(retained, root);
            }
        }

        if self.can_fast_path_clean(root, viewport) {
            self.stats.cache_hits += 1;
            let retained = self.retained.as_mut().expect("retained tree must exist");
            if retained.root() != root {
                retained.sync_root(root.clone());
            }
            let display_list = retained.display_list().snapshot(viewport);
            return ComposeUpdate::Full {
                display_list,
                compose_stats: self.stats,
            };
        }

        if self.can_fast_path_paint(viewport) {
            self.stats.compose_passes += 1;
            let retained = self.retained.as_mut().expect("retained tree must exist");
            let dirty_indices: HashSet<usize> = retained.dirty_indices().into_iter().collect();
            let previous_dirty_bounds = retained
                .display_list()
                .bounds_for_object_indices(dirty_indices.iter().copied());
            let image_resources =
                ImageResourceTable::from_frontend(retained.layout().object_table().as_ref());
            retained.replace_image_resources(image_resources.clone());
            let rebuilt_display_list = display_list::build_retained_display_list(
                root,
                retained.layout(),
                &image_resources,
                viewport,
            );
            retained.replace_display_list(rebuilt_display_list);
            retained.sync_root(root.clone());
            let current_dirty_bounds = retained
                .display_list()
                .bounds_for_object_indices(dirty_indices.iter().copied());
            let dirty_bounds = merge_bounds(previous_dirty_bounds, current_dirty_bounds);
            let display_list = retained.display_list().snapshot(viewport);
            if dirty_indices.is_empty() {
                return ComposeUpdate::Full {
                    display_list,
                    compose_stats: self.stats,
                };
            }
            return ComposeUpdate::Delta {
                dirty_bounds,
                patch_upserts: dirty_indices.len(),
                patch_removes: 0,
                display_list,
                compose_stats: self.stats,
            };
        }

        if self.can_fast_path_layout(viewport) {
            self.stats.compose_passes += 1;
            self.stats.layout_passes += 1;
            let retained = self.retained.as_mut().expect("retained tree must exist");
            let layout_dirty_roots = retained.layout_dirty_root_indices();
            let layout = relayout::relayout_layout(
                root,
                Point::new(0.0, 0.0),
                viewport,
                self.text_system,
                retained,
                &layout_dirty_roots,
            );
            let available = fragments::available_slots_from_layout(root, viewport, &layout);
            retained.apply_layout_state(root.clone(), viewport, layout.clone(), available);
            let image_resources = ImageResourceTable::from_frontend(layout.object_table().as_ref());
            retained.replace_image_resources(image_resources.clone());
            let rebuilt_display_list = display_list::build_retained_display_list(
                root,
                &layout,
                &image_resources,
                viewport,
            );
            retained.replace_display_list(rebuilt_display_list);
            let display_list = retained.display_list().snapshot(viewport);
            return ComposeUpdate::Full {
                display_list,
                compose_stats: self.stats,
            };
        }

        self.stats.compose_passes += 1;
        self.stats.layout_passes += 1;
        let layout = measure_layout(root, Point::new(0.0, 0.0), viewport, self.text_system);
        let available = fragments::available_slots_from_layout(root, viewport, &layout);
        let image_resources = ImageResourceTable::from_frontend(layout.object_table().as_ref());
        let display_list =
            display_list::build_retained_display_list(root, &layout, &image_resources, viewport);
        match self.retained.as_mut() {
            Some(retained) => retained.replace(
                root.clone(),
                viewport,
                layout,
                available,
                image_resources,
                display_list,
            ),
            None => {
                self.retained = Some(RetainedComposeTree::new(
                    root.clone(),
                    viewport,
                    layout,
                    available,
                    image_resources,
                    display_list,
                ));
            }
        }
        let retained = self
            .retained
            .as_ref()
            .expect("retained display list should exist after full compose");
        let display_list = retained.display_list().snapshot(viewport);
        ComposeUpdate::Full {
            display_list,
            compose_stats: self.stats,
        }
    }

    fn can_fast_path_clean(&self, root: &Node, viewport: Size) -> bool {
        match self.retained.as_ref() {
            Some(retained) => {
                retained.dirty().is_clean()
                    && retained.viewport() == viewport
                    && retained.root() == root
            }
            None => false,
        }
    }

    fn can_fast_path_paint(&self, viewport: Size) -> bool {
        match self.retained.as_ref() {
            Some(retained) => {
                retained.dirty().requires_paint_only() && retained.viewport() == viewport
            }
            None => false,
        }
    }

    fn can_fast_path_layout(&self, viewport: Size) -> bool {
        match self.retained.as_ref() {
            Some(retained) => retained.dirty().requires_layout() && retained.viewport() == viewport,
            None => false,
        }
    }

    pub fn invalidate(&mut self, reason: DirtyReason) {
        if let Some(retained) = self.retained.as_mut() {
            retained.mark_dirty(reason);
        }
    }

    pub fn invalidate_node(&mut self, node_id: NodeId, reason: DirtyReason) {
        if let Some(retained) = self.retained.as_mut() {
            retained.mark_node_dirty(node_id, reason);
        }
    }

    #[must_use]
    pub fn current_display_list(&self) -> Option<&zeno_scene::RetainedDisplayList> {
        self.retained
            .as_ref()
            .map(RetainedComposeTree::display_list)
    }

    #[must_use]
    pub const fn stats(&self) -> ComposeStats {
        self.stats
    }
}

#[must_use]
pub fn dump_layout(root: &Node, viewport: Size, text_system: &dyn TextSystem) -> String {
    debug::dump_layout(root, viewport, text_system)
}

fn merge_bounds(first: Option<Rect>, second: Option<Rect>) -> Option<Rect> {
    match (first, second) {
        (Some(first), Some(second)) => Some(first.union(&second)),
        (Some(first), None) => Some(first),
        (None, Some(second)) => Some(second),
        (None, None) => None,
    }
}

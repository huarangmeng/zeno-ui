//! render 模块按职责拆分，避免单文件继续膨胀。

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use zeno_core::{Point, Rect, Size, Transform2D};
use zeno_scene::{
    Brush, DisplayList, LayerOrder, LayerObject, RenderObject, RenderObjectDelta,
    RenderObjectOrder, RetainedScene, Scene, SceneBlendMode, SceneClip, SceneEffect,
    SceneTransform, Shape,
};
use zeno_text::TextSystem;

use crate::{
    Node, NodeId, NodeKind,
    invalidation::DirtyReason,
    layout::measure_layout,
    modifier::{BlendMode, ClipMode, TransformOrigin},
    tree::RetainedComposeTree,
};

mod debug;
mod display_list;
mod fragments;
mod patch;
mod reconcile;
mod relayout;
mod scene;


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
    pub fn compose(&self, root: &Node, viewport: Size) -> Scene {
        debug::compose_scene_internal(root, viewport, self.text_system)
    }

    #[must_use]
    pub fn compose_display_list(&self, root: &Node, viewport: Size) -> DisplayList {
        let layout = measure_layout(root, Point::new(0.0, 0.0), viewport, self.text_system);
        let retained = display_list::build_retained_display_list(root, &layout, viewport);
        display_list::snapshot_display_list(&retained, viewport)
    }
}

pub struct ComposeEngine<'a> {
    text_system: &'a dyn TextSystem,
    retained: Option<RetainedComposeTree>,
    stats: ComposeStats,
}

pub enum RetainedComposeUpdate<'a> {
    Full {
        scene: &'a mut RetainedScene,
        display_list: DisplayList,
        compose_stats: ComposeStats,
    },
    Delta {
        scene: &'a mut RetainedScene,
        delta: RenderObjectDelta,
        dirty_bounds: Option<Rect>,
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
    pub fn compose(&mut self, root: &Node, viewport: Size) -> Scene {
        match self.compose_submit_retained(root, viewport) {
            RetainedComposeUpdate::Full { scene, .. } => scene.snapshot_scene(),
            RetainedComposeUpdate::Delta { scene, .. } => scene.snapshot_scene(),
        }
    }

    #[must_use]
    pub fn compose_display_list(&mut self, root: &Node, viewport: Size) -> DisplayList {
        let _ = self.compose_submit_retained(root, viewport);
        self.current_display_list()
            .expect("display list should exist after compose")
            .snapshot(viewport)
    }

    pub fn compose_submit_retained(
        &mut self,
        root: &Node,
        viewport: Size,
    ) -> RetainedComposeUpdate<'_> {
        if let Some(retained) = self.retained.as_mut() {
            if retained.scene().size == viewport && retained.root() != root {
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
            return RetainedComposeUpdate::Full {
                scene: retained.scene_mut(),
                display_list,
                compose_stats: self.stats,
            };
        }

        if self.can_fast_path_paint(viewport) {
            self.stats.compose_passes += 1;
            let retained = self.retained.as_mut().expect("retained tree must exist");
            let dirty_indices: HashSet<usize> = retained.dirty_indices().into_iter().collect();
            patch::repaint_dirty_nodes(root, retained);
            let rebuilt_display_list =
                display_list::build_retained_display_list(root, retained.layout(), viewport);
            retained.replace_display_list(rebuilt_display_list);
            let patch = patch::patch_scene_for_nodes(root, retained, &dirty_indices);
            let dirty_bounds = retained.scene_mut().dirty_bounds_for_delta(&patch);
            retained.sync_root(root.clone());
            let display_list = retained.display_list().snapshot(viewport);
            if patch.is_empty() {
                return RetainedComposeUpdate::Full {
                    scene: retained.scene_mut(),
                    display_list,
                    compose_stats: self.stats,
                };
            }
            return RetainedComposeUpdate::Delta {
                scene: retained.scene_mut(),
                delta: patch,
                dirty_bounds,
                display_list,
                compose_stats: self.stats,
            };
        }

        if self.can_fast_path_layout(viewport) {
            self.stats.compose_passes += 1;
            self.stats.layout_passes += 1;
            let retained = self.retained.as_mut().expect("retained tree must exist");
            let dirty_indices: HashSet<usize> = retained.dirty_indices().into_iter().collect();
            let previous_node_ids: HashSet<NodeId> = retained.node_ids().iter().copied().collect();
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
            let current_node_ids: HashSet<NodeId> =
                layout.object_table().node_ids().iter().copied().collect();
            let new_indices: HashSet<usize> = current_node_ids
                .difference(&previous_node_ids)
                .filter_map(|id| layout.object_table().index_of(*id))
                .collect();
            let fragment_update_ids: HashSet<usize> =
                dirty_indices.union(&new_indices).copied().collect();
            let patch_update_ids = patch::scene_update_ids_for_relayout(
                root,
                &layout,
                retained,
                &fragment_update_ids,
            );
            retained.apply_layout_state(root.clone(), viewport, layout.clone(), available);
            patch::update_fragments_for_nodes(
                root,
                &layout,
                viewport,
                &fragment_update_ids,
                retained,
            );
            let rebuilt_display_list =
                display_list::build_retained_display_list(root, &layout, viewport);
            retained.replace_display_list(rebuilt_display_list);
            let patch = patch::patch_scene_for_nodes(root, retained, &patch_update_ids);
            let dirty_bounds = retained.scene_mut().dirty_bounds_for_delta(&patch);
            let display_list = retained.display_list().snapshot(viewport);
            if patch.is_empty() {
                return RetainedComposeUpdate::Full {
                    scene: retained.scene_mut(),
                    display_list,
                    compose_stats: self.stats,
                };
            }
            return RetainedComposeUpdate::Delta {
                scene: retained.scene_mut(),
                delta: patch,
                dirty_bounds,
                display_list,
                compose_stats: self.stats,
            };
        }

        self.stats.compose_passes += 1;
        self.stats.layout_passes += 1;
        let layout = measure_layout(root, Point::new(0.0, 0.0), viewport, self.text_system);
        let available = fragments::available_slots_from_layout(root, viewport, &layout);
        let scene = scene::build_scene(root, &layout, viewport);
        let display_list = display_list::build_retained_display_list(root, &layout, viewport);
        match self.retained.as_mut() {
            Some(retained) => retained.replace(
                root.clone(),
                viewport,
                layout,
                available,
                display_list,
                scene.clone(),
            ),
            None => {
                self.retained = Some(RetainedComposeTree::new(
                    root.clone(),
                    viewport,
                    layout,
                    available,
                    display_list,
                    scene.clone(),
                ));
            }
        }
        let retained = self
            .retained
            .as_mut()
            .expect("retained scene should exist after full compose");
        let display_list = retained.display_list().snapshot(viewport);
        RetainedComposeUpdate::Full {
            scene: retained.scene_mut(),
            display_list,
            compose_stats: self.stats,
        }
    }

    fn can_fast_path_clean(&self, root: &Node, viewport: Size) -> bool {
        match self.retained.as_ref() {
            Some(retained) => {
                retained.dirty().is_clean()
                    && retained.scene().size == viewport
                    && retained.root() == root
            }
            None => false,
        }
    }

    fn can_fast_path_paint(&self, viewport: Size) -> bool {
        match self.retained.as_ref() {
            Some(retained) => retained.dirty().requires_paint_only() && retained.scene().size == viewport,
            None => false,
        }
    }

    fn can_fast_path_layout(&self, viewport: Size) -> bool {
        match self.retained.as_ref() {
            Some(retained) => retained.dirty().requires_layout() && retained.scene().size == viewport,
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
    pub fn current_scene(&self) -> Option<&RetainedScene> {
        self.retained.as_ref().map(RetainedComposeTree::scene)
    }

    #[must_use]
    pub fn current_display_list(&self) -> Option<&zeno_scene::RetainedDisplayList> {
        self.retained.as_ref().map(RetainedComposeTree::display_list)
    }

    #[must_use]
    pub const fn stats(&self) -> ComposeStats {
        self.stats
    }
}

#[must_use]
pub fn compose_scene(root: &Node, viewport: Size, text_system: &dyn TextSystem) -> Scene {
    debug::compose_scene_internal(root, viewport, text_system)
}

#[must_use]
pub fn dump_scene(scene: &Scene) -> String {
    debug::dump_scene(scene)
}

#[must_use]
pub fn dump_layout(root: &Node, viewport: Size, text_system: &dyn TextSystem) -> String {
    debug::dump_layout(root, viewport, text_system)
}

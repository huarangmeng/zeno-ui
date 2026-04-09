//! render 模块按职责拆分，避免单文件继续膨胀。

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use zeno_core::{Point, Rect, Size, Transform2D};
use zeno_scene::{
    Brush, DrawCommand, Scene, SceneBlendMode, SceneBlock, SceneBlockOrder, SceneClip, SceneEffect,
    SceneLayerOrder, ScenePatch, SceneSubmit, SceneTransform, Shape,
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
mod fragments;
mod patch;
mod reconcile;
mod relayout;
mod scene;

pub(crate) use fragments::FragmentStore;

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
}

pub struct ComposeEngine<'a> {
    text_system: &'a dyn TextSystem,
    retained: Option<RetainedComposeTree>,
    stats: ComposeStats,
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
        match self.compose_submit(root, viewport) {
            SceneSubmit::Full(scene) => scene,
            SceneSubmit::Patch { current, .. } => current,
        }
    }

    #[must_use]
    pub fn compose_submit(&mut self, root: &Node, viewport: Size) -> SceneSubmit {
        if let Some(retained) = self.retained.as_mut() {
            if retained.scene().size == viewport && retained.root() != root {
                reconcile::reconcile_root_change(retained, root);
            }
        }

        if let Some(retained) = self.retained.as_mut() {
            if retained.dirty().is_clean() && retained.scene().size == viewport {
                if retained.root() != root {
                    retained.sync_root(root.clone());
                }
                self.stats.cache_hits += 1;
                return SceneSubmit::Full(retained.scene().clone());
            }
        }

        if let Some(retained) = self.retained.as_mut() {
            if retained.dirty().requires_paint_only() && retained.scene().size == viewport {
                self.stats.compose_passes += 1;
                let dirty_node_ids: HashSet<NodeId> =
                    retained.dirty_node_ids().into_iter().collect();
                patch::repaint_dirty_nodes(root, retained);
                let patch = patch::patch_scene_for_nodes(root, retained, &dirty_node_ids);
                let scene = retained.scene().clone();
                retained.sync_root(root.clone());
                return if patch.is_empty() {
                    SceneSubmit::Full(scene)
                } else {
                    SceneSubmit::Patch {
                        patch,
                        current: scene,
                    }
                };
            }
        }

        if let Some(retained) = self.retained.as_mut() {
            if retained.dirty().requires_layout() && retained.scene().size == viewport {
                self.stats.compose_passes += 1;
                self.stats.layout_passes += 1;
                let dirty_node_ids: HashSet<NodeId> =
                    retained.dirty_node_ids().into_iter().collect();
                let previous_node_ids: HashSet<NodeId> =
                    retained.node_ids().iter().copied().collect();
                let layout_dirty_roots = retained.layout_dirty_roots();
                let layout = relayout::relayout_layout(
                    root,
                    Point::new(0.0, 0.0),
                    viewport,
                    self.text_system,
                    retained,
                    &layout_dirty_roots,
                );
                let available_by_node =
                    fragments::available_map_from_layout(root, viewport, &layout);
                let current_node_ids: HashSet<NodeId> = available_by_node.keys().copied().collect();
                let new_node_ids: HashSet<NodeId> = current_node_ids
                    .difference(&previous_node_ids)
                    .copied()
                    .collect();
                let fragment_update_ids: HashSet<NodeId> =
                    dirty_node_ids.union(&new_node_ids).copied().collect();
                let patch_update_ids = patch::scene_update_ids_for_relayout(
                    root,
                    &layout,
                    retained,
                    &fragment_update_ids,
                );
                retained.apply_layout_state(
                    root.clone(),
                    viewport,
                    layout.clone(),
                    available_by_node,
                );
                patch::update_fragments_for_nodes(
                    root,
                    &layout,
                    viewport,
                    &fragment_update_ids,
                    retained,
                );
                let patch = patch::patch_scene_for_nodes(root, retained, &patch_update_ids);
                let scene = retained.scene().clone();
                return if patch.is_empty() {
                    SceneSubmit::Full(scene)
                } else {
                    SceneSubmit::Patch {
                        patch,
                        current: scene,
                    }
                };
            }
        }

        self.stats.compose_passes += 1;
        self.stats.layout_passes += 1;
        let layout = measure_layout(root, Point::new(0.0, 0.0), viewport, self.text_system);
        let (available_by_node, fragments_by_node, scene) =
            fragments::structured_scene_from_layout(root, viewport, &layout);
        match self.retained.as_mut() {
            Some(retained) => retained.replace(
                root.clone(),
                viewport,
                layout,
                available_by_node,
                fragments_by_node,
                scene.clone(),
            ),
            None => {
                self.retained = Some(RetainedComposeTree::new(
                    root.clone(),
                    viewport,
                    layout,
                    available_by_node,
                    fragments_by_node,
                    scene.clone(),
                ));
            }
        }
        SceneSubmit::Full(scene)
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
    pub fn current_scene(&self) -> Option<&Scene> {
        self.retained.as_ref().map(RetainedComposeTree::scene)
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

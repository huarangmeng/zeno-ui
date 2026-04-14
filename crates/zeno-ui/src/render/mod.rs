//! render 模块按职责拆分，避免单文件继续膨胀。

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use zeno_core::{Point, Size};
use zeno_scene::{DamageRegion, DamageTracker, DisplayList, RetainedDisplayList};
use zeno_text::TextSystem;

use crate::{
    InteractionState, Node, NodeId, NodeKind, frontend::ElementId, image::ImageResourceTable,
    invalidation::DirtyReason, layout::measure_layout, modifier::BlendMode,
    tree::RetainedComposeTree,
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
        damage: DamageRegion,
        patch_upserts: usize,
        patch_removes: usize,
        display_list: DisplayList,
        compose_stats: ComposeStats,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractionTarget {
    pub node_id: NodeId,
    pub element_id: ElementId,
    pub interaction: InteractionState,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InteractionTargetFrame {
    pub node_id: NodeId,
    pub element_id: ElementId,
    pub interaction: InteractionState,
    pub frame: zeno_core::Rect,
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
            let dirty_element_ids: Vec<_> = dirty_indices
                .iter()
                .copied()
                .map(|index| retained.layout().object_table().element_id_at(index))
                .collect();
            let previous_display_list = retained.display_list().clone();
            let previous_object_table = retained.layout().object_table().clone();
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
            let damage = damage_for_element_ids(
                &previous_display_list,
                previous_object_table.as_ref(),
                retained.display_list(),
                retained.layout().object_table().as_ref(),
                dirty_element_ids.iter().copied(),
            );
            let display_list = retained.display_list().snapshot(viewport);
            if dirty_indices.is_empty() {
                return ComposeUpdate::Full {
                    display_list,
                    compose_stats: self.stats,
                };
            }
            return ComposeUpdate::Delta {
                damage,
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
            let layout_dirty_element_ids: Vec<_> = layout_dirty_roots
                .iter()
                .copied()
                .map(|index| retained.layout().object_table().element_id_at(index))
                .collect();
            let previous_display_list = retained.display_list().clone();
            let previous_object_table = retained.layout().object_table().clone();
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
            let damage = damage_for_element_ids(
                &previous_display_list,
                previous_object_table.as_ref(),
                retained.display_list(),
                retained.layout().object_table().as_ref(),
                layout_dirty_element_ids.iter().copied(),
            );
            if damage.is_empty() {
                return ComposeUpdate::Full {
                    display_list,
                    compose_stats: self.stats,
                };
            }
            return ComposeUpdate::Delta {
                damage,
                patch_upserts: layout_dirty_element_ids.len(),
                patch_removes: 0,
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
    pub fn hit_test(&self, point: Point) -> Option<InteractionTarget> {
        let retained = self.retained.as_ref()?;
        let layout = retained.layout();
        let objects = retained.objects();
        (0..objects.len()).rev().find_map(|index| {
            let object = objects.object(index);
            if !object.interaction.is_interactive() {
                return None;
            }
            let frame = layout.slot_at(index).frame;
            let contains = point.x >= frame.origin.x
                && point.x <= frame.origin.x + frame.size.width
                && point.y >= frame.origin.y
                && point.y <= frame.origin.y + frame.size.height;
            contains.then_some(InteractionTarget {
                node_id: object.node_id,
                element_id: object.element_id,
                interaction: object.interaction,
            })
        })
    }

    #[must_use]
    pub fn interaction_target(&self, node_id: NodeId) -> Option<InteractionTarget> {
        let retained = self.retained.as_ref()?;
        let objects = retained.objects();
        let index = objects.index_of(node_id)?;
        let object = objects.object(index);
        Some(InteractionTarget {
            node_id: object.node_id,
            element_id: object.element_id,
            interaction: object.interaction,
        })
    }

    #[must_use]
    pub fn interaction_target_by_element(&self, element_id: ElementId) -> Option<InteractionTarget> {
        let retained = self.retained.as_ref()?;
        let objects = retained.objects();
        (0..objects.len()).find_map(|index| {
            let object = objects.object(index);
            (object.element_id == element_id).then_some(InteractionTarget {
                node_id: object.node_id,
                element_id: object.element_id,
                interaction: object.interaction,
            })
        })
    }

    #[must_use]
    pub fn focusable_targets(&self) -> Vec<InteractionTarget> {
        let Some(retained) = self.retained.as_ref() else {
            return Vec::new();
        };
        let objects = retained.objects();
        (0..objects.len())
            .filter_map(|index| {
                let object = objects.object(index);
                object
                    .interaction
                    .is_focusable()
                    .then_some(InteractionTarget {
                        node_id: object.node_id,
                        element_id: object.element_id,
                        interaction: object.interaction,
                    })
            })
            .collect()
    }

    #[must_use]
    pub fn interactive_target_frames(&self) -> Vec<InteractionTargetFrame> {
        let Some(retained) = self.retained.as_ref() else {
            return Vec::new();
        };
        let objects = retained.objects();
        let layout = retained.layout();
        (0..objects.len())
            .filter_map(|index| {
                let object = objects.object(index);
                object
                    .interaction
                    .is_interactive()
                    .then_some(InteractionTargetFrame {
                        node_id: object.node_id,
                        element_id: object.element_id,
                        interaction: object.interaction,
                        frame: layout.slot_at(index).frame,
                    })
            })
            .collect()
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

fn damage_for_element_ids(
    previous_display_list: &RetainedDisplayList,
    previous_object_table: &crate::frontend::FrontendObjectTable,
    current_display_list: &RetainedDisplayList,
    current_object_table: &crate::frontend::FrontendObjectTable,
    element_ids: impl IntoIterator<Item = ElementId>,
) -> DamageRegion {
    let mut damage = DamageTracker::new();
    for element_id in element_ids {
        let previous_bounds =
            subtree_bounds_for_element_id(previous_display_list, previous_object_table, element_id);
        let current_bounds =
            subtree_bounds_for_element_id(current_display_list, current_object_table, element_id);
        damage.add_optional_rect(previous_bounds);
        damage.add_optional_rect(current_bounds);
    }
    damage.build()
}

fn subtree_bounds_for_element_id(
    display_list: &RetainedDisplayList,
    object_table: &crate::frontend::FrontendObjectTable,
    element_id: ElementId,
) -> Option<zeno_core::Rect> {
    let root_index = object_table.index_of_element(element_id)?;
    let mut subtree_indices = Vec::new();
    collect_subtree_indices(object_table, root_index, &mut subtree_indices);
    display_list.bounds_for_object_indices(subtree_indices)
}

fn collect_subtree_indices(
    object_table: &crate::frontend::FrontendObjectTable,
    root_index: usize,
    out: &mut Vec<usize>,
) {
    out.push(root_index);
    for &child_index in object_table.child_indices(root_index) {
        collect_subtree_indices(object_table, child_index, out);
    }
}

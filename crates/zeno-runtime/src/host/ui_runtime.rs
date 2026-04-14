use zeno_core::{Size, ZenoError, ZenoErrorCode};
use zeno_scene::DisplayList;
use zeno_text::TextSystem;
use zeno_ui::{ComposeEngine, ComposeStats, DirtyReason, Node, NodeId};

use crate::{FramePhases, FrameScheduler};

pub struct UiFrame {
    pub scene_update: UiSceneUpdate,
    pub phases: FramePhases,
    pub compose_stats: ComposeStats,
}

pub enum UiSceneUpdate {
    Full {
        display_list: DisplayList,
        compose_stats: ComposeStats,
    },
    Delta {
        dirty_bounds: Option<zeno_core::Rect>,
        patch_upserts: usize,
        patch_removes: usize,
        display_list: DisplayList,
        compose_stats: ComposeStats,
    },
}

impl UiFrame {
    #[must_use]
    pub fn display_list(&self) -> &DisplayList {
        self.scene_update.display_list()
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        matches!(self.scene_update, UiSceneUpdate::Full { .. })
    }
}

impl UiSceneUpdate {
    #[must_use]
    pub fn display_list(&self) -> &DisplayList {
        match self {
            UiSceneUpdate::Full { display_list, .. } => display_list,
            UiSceneUpdate::Delta { display_list, .. } => display_list,
        }
    }
}

pub struct UiRuntime<'a> {
    engine: ComposeEngine<'a>,
    scheduler: FrameScheduler,
    root: Option<Node>,
    viewport: Option<Size>,
}

impl<'a> UiRuntime<'a> {
    #[must_use]
    pub fn new(text_system: &'a dyn TextSystem) -> Self {
        Self {
            engine: ComposeEngine::new(text_system),
            scheduler: FrameScheduler::new(),
            root: None,
            viewport: None,
        }
    }

    pub fn set_root(&mut self, root: Node) {
        if self.root.as_ref() != Some(&root) {
            let had_root = self.root.is_some();
            self.root = Some(root);
            if had_root {
                self.scheduler.invalidate_paint();
            } else {
                self.scheduler.invalidate_layout();
            }
        }
    }

    pub fn resize(&mut self, viewport: Size) {
        if self.viewport != Some(viewport) {
            self.viewport = Some(viewport);
            self.scheduler.invalidate_layout();
        }
    }

    pub fn request_paint(&mut self) {
        self.scheduler.invalidate_paint();
    }

    pub fn request_node_paint(&mut self, node_id: NodeId) {
        self.engine.invalidate_node(node_id, DirtyReason::Paint);
        self.scheduler.invalidate_paint();
    }

    #[must_use]
    pub fn has_pending_frame(&self) -> bool {
        self.scheduler.has_pending_frame()
    }

    pub fn prepare_frame(&mut self) -> Result<Option<UiFrame>, ZenoError> {
        if !self.scheduler.has_pending_frame() {
            return Ok(None);
        }

        let root = self.root.as_ref().ok_or_else(|| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::UiRuntimeRootNotSet,
                "ui.runtime",
                "prepare_frame",
                "ui runtime root is not set",
            )
        })?;
        let viewport = self.viewport.ok_or_else(|| {
            ZenoError::invalid_configuration(
                ZenoErrorCode::UiRuntimeViewportNotConfigured,
                "ui.runtime",
                "prepare_frame",
                "ui runtime viewport is not configured",
            )
        })?;
        let phases = self.scheduler.pending();

        if phases.needs_layout {
            self.engine.invalidate(DirtyReason::Layout);
        } else if phases.needs_paint {
            self.engine.invalidate(DirtyReason::Paint);
        }

        let scene_update = match self.engine.compose_update(root, viewport) {
            zeno_ui::ComposeUpdate::Full {
                display_list,
                compose_stats,
            } => UiSceneUpdate::Full {
                display_list,
                compose_stats,
            },
            zeno_ui::ComposeUpdate::Delta {
                dirty_bounds,
                patch_upserts,
                patch_removes,
                display_list,
                compose_stats,
            } => UiSceneUpdate::Delta {
                dirty_bounds,
                patch_upserts,
                patch_removes,
                display_list,
                compose_stats,
            },
        };
        let compose_stats = match &scene_update {
            UiSceneUpdate::Full { compose_stats, .. } => *compose_stats,
            UiSceneUpdate::Delta { compose_stats, .. } => *compose_stats,
        };
        let frame = UiFrame {
            scene_update,
            phases,
            compose_stats,
        };
        self.scheduler.finish_frame();
        Ok(Some(frame))
    }

    pub fn request_node_layout(&mut self, node_id: NodeId) {
        self.engine.invalidate_node(node_id, DirtyReason::Layout);
        self.scheduler.invalidate_layout();
    }
}

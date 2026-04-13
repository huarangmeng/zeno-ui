use zeno_ui::{ComposeEngine, ComposeStats, DirtyReason, Node, NodeId};
use zeno_core::{Size, ZenoError, ZenoErrorCode};
use zeno_scene::{DisplayList, RenderObjectDelta, RetainedScene};
use zeno_text::TextSystem;

use crate::{FramePhases, FrameScheduler};

pub struct UiFrame<'a> {
    pub scene_update: UiSceneUpdate<'a>,
    pub phases: FramePhases,
    pub compose_stats: ComposeStats,
}

pub enum UiSceneUpdate<'a> {
    Full {
        scene: &'a mut RetainedScene,
        display_list: DisplayList,
        compose_stats: ComposeStats,
    },
    Delta {
        scene: &'a mut RetainedScene,
        delta: RenderObjectDelta,
        dirty_bounds: Option<zeno_core::Rect>,
        display_list: DisplayList,
        compose_stats: ComposeStats,
    },
}

impl<'a> UiFrame<'a> {
    #[must_use]
    pub fn scene(&self) -> &RetainedScene {
        self.scene_update.scene()
    }

    #[must_use]
    pub fn display_list(&self) -> &DisplayList {
        self.scene_update.display_list()
    }

    pub fn scene_mut(&mut self) -> &mut RetainedScene {
        self.scene_update.scene_mut()
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        matches!(self.scene_update, UiSceneUpdate::Full { .. })
    }
}

impl<'a> UiSceneUpdate<'a> {
    #[must_use]
    pub fn scene(&self) -> &RetainedScene {
        match self {
            UiSceneUpdate::Full { scene, .. } => scene,
            UiSceneUpdate::Delta { scene, .. } => scene,
        }
    }

    #[must_use]
    pub fn display_list(&self) -> &DisplayList {
        match self {
            UiSceneUpdate::Full { display_list, .. } => display_list,
            UiSceneUpdate::Delta { display_list, .. } => display_list,
        }
    }

    pub fn scene_mut(&mut self) -> &mut RetainedScene {
        match self {
            UiSceneUpdate::Full { scene, .. } => scene,
            UiSceneUpdate::Delta { scene, .. } => scene,
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

    pub fn prepare_frame(&mut self) -> Result<Option<UiFrame<'_>>, ZenoError> {
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

        let scene_update = match self.engine.compose_submit_retained(root, viewport) {
            zeno_ui::RetainedComposeUpdate::Full {
                scene,
                display_list,
                compose_stats,
            } => UiSceneUpdate::Full {
                scene,
                display_list,
                compose_stats,
            },
            zeno_ui::RetainedComposeUpdate::Delta {
                scene,
                delta,
                dirty_bounds,
                display_list,
                compose_stats,
            } => UiSceneUpdate::Delta {
                scene,
                delta,
                dirty_bounds,
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

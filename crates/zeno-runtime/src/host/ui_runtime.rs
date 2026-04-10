use zeno_ui::{ComposeEngine, ComposeStats, DirtyReason, Node, NodeId};
use zeno_core::{Size, ZenoError, ZenoErrorCode};
use zeno_scene::{RenderSceneUpdate, Scene};
use zeno_text::TextSystem;

use crate::{FramePhases, FrameScheduler};

#[derive(Debug, Clone, PartialEq)]
pub struct UiFrame {
    pub scene: Scene,
    pub scene_update: RenderSceneUpdate,
    pub phases: FramePhases,
    pub compose_stats: ComposeStats,
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

        let scene_update = self.engine.compose_submit(root, viewport);
        let scene = match &scene_update {
            RenderSceneUpdate::Full(scene) => scene.clone(),
            RenderSceneUpdate::Delta { current, .. } => current.clone(),
        };
        let frame = UiFrame {
            scene,
            scene_update,
            phases,
            compose_stats: self.engine.stats(),
        };
        self.scheduler.finish_frame();
        Ok(Some(frame))
    }

    pub fn request_node_layout(&mut self, node_id: NodeId) {
        self.engine.invalidate_node(node_id, DirtyReason::Layout);
        self.scheduler.invalidate_layout();
    }
}

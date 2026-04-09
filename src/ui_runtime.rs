use zeno_compose::{ComposeEngine, ComposeStats, DirtyReason, Node, NodeId};
use zeno_core::{AppConfig, Platform, Size, ZenoError, ZenoErrorCode, zeno_runtime_log};
use zeno_graphics::{Scene, SceneSubmit};
use zeno_runtime::{FramePhases, FrameScheduler, ResolvedSession};
use zeno_text::TextSystem;

#[derive(Debug, Clone, PartialEq)]
pub struct UiFrame {
    pub scene: Scene,
    pub scene_submit: SceneSubmit,
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

        let root = self
            .root
            .as_ref()
            .ok_or_else(|| {
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

        let scene_submit = self.engine.compose_submit(root, viewport);
        let scene = match &scene_submit {
            SceneSubmit::Full(scene) => scene.clone(),
            SceneSubmit::Patch { current, .. } => current.clone(),
        };
        let frame = UiFrame {
            scene,
            scene_submit,
            phases,
            compose_stats: self.engine.stats(),
        };
        self.scheduler.finish_frame();
        Ok(Some(frame))
    }

    pub fn prepare_resolved_frame(
        &mut self,
        platform: Platform,
        app_config: &AppConfig,
    ) -> Result<Option<(ResolvedSession, UiFrame)>, ZenoError> {
        let Some(frame) = self.prepare_frame()? else {
            return Ok(None);
        };
        let session = ResolvedSession::resolve(platform, app_config)?;
        zeno_runtime_log!(
            trace,
            app = %app_config.app_name,
            platform = ?platform,
            backend = ?session.backend.backend_kind,
            attempts = session.backend.attempts.len(),
            frame_stats = session.frame_stats,
            "resolved backend session"
        );
        Ok(Some((session, frame)))
    }

    pub fn request_node_layout(&mut self, node_id: NodeId) {
        self.engine.invalidate_node(node_id, DirtyReason::Layout);
        self.scheduler.invalidate_layout();
    }
}

#[cfg(test)]
mod tests {
    use super::UiRuntime;
    use crate::{column, text, Backend, FallbackTextSystem, Size};
    use zeno_core::Color;
    use zeno_graphics::SceneSubmit;

    #[test]
    fn runtime_reuses_cached_scene_for_paint_requests() {
        let mut runtime = UiRuntime::new(&FallbackTextSystem);
        runtime.set_root(column(vec![text("Hello"), text("World")]));
        runtime.resize(Size::new(320.0, 240.0));

        let first = runtime.prepare_frame().expect("frame").expect("scene");
        assert_eq!(first.compose_stats.compose_passes, 1);
        assert_eq!(first.compose_stats.layout_passes, 1);

        runtime.request_paint();
        let second = runtime.prepare_frame().expect("frame").expect("scene");

        assert_eq!(second.compose_stats.compose_passes, 2);
        assert_eq!(second.compose_stats.layout_passes, 1);
        assert_eq!(second.scene.commands.len(), first.scene.commands.len());
    }

    #[test]
    fn runtime_can_prepare_resolved_frame() {
        let mut runtime = UiRuntime::new(&FallbackTextSystem);
        runtime.set_root(column(vec![text("Hello")]));
        runtime.resize(Size::new(320.0, 240.0));

        let config = zeno_core::AppConfig::default();
        let (session, frame) = runtime
            .prepare_resolved_frame(zeno_core::Platform::Linux, &config)
            .expect("resolved frame")
            .expect("pending frame");

        assert_eq!(session.backend.backend_kind, Backend::Skia);
        assert!(frame.compose_stats.compose_passes >= 1);
    }

    #[test]
    fn runtime_can_repaint_single_node_without_new_layout() {
        let title = text("Hello");
        let title_id = title.id();
        let mut runtime = UiRuntime::new(&FallbackTextSystem);
        runtime.set_root(column(vec![title, text("World")]));
        runtime.resize(Size::new(320.0, 240.0));

        let first = runtime.prepare_frame().expect("frame").expect("scene");
        runtime.request_node_paint(title_id);
        let second = runtime.prepare_frame().expect("frame").expect("scene");

        assert_eq!(first.scene.commands.len(), second.scene.commands.len());
        assert_eq!(second.compose_stats.layout_passes, 1);
    }

    #[test]
    fn runtime_node_layout_request_triggers_new_layout_pass() {
        let title = text("Hello");
        let title_id = title.id();
        let mut runtime = UiRuntime::new(&FallbackTextSystem);
        runtime.set_root(column(vec![title, text("World")]));
        runtime.resize(Size::new(320.0, 240.0));

        let first = runtime.prepare_frame().expect("frame").expect("scene");
        runtime.request_node_layout(title_id);
        let second = runtime.prepare_frame().expect("frame").expect("scene");

        assert_eq!(second.compose_stats.layout_passes, first.compose_stats.layout_passes + 1);
    }

    #[test]
    fn runtime_downgrades_keyed_root_rebuild_to_paint_patch() {
        let mut runtime = UiRuntime::new(&FallbackTextSystem);
        runtime.set_root(
            column(vec![text("Hello").key("title"), text("World").key("body")])
                .spacing(4.0)
                .key("root"),
        );
        runtime.resize(Size::new(320.0, 240.0));

        let first = runtime.prepare_frame().expect("frame").expect("scene");
        runtime.set_root(
            column(vec![
                text("Hello").key("title").foreground(Color::WHITE),
                text("World").key("body"),
            ])
            .spacing(4.0)
            .key("root"),
        );
        let second = runtime.prepare_frame().expect("frame").expect("scene");

        assert_eq!(second.compose_stats.layout_passes, first.compose_stats.layout_passes);
        match second.scene_submit {
            SceneSubmit::Patch { patch, .. } => {
                assert_eq!(patch.upserts.len(), 1);
                assert!(patch.removes.is_empty());
            }
            SceneSubmit::Full(_) => panic!("expected patch submit"),
        }
    }

    #[test]
    fn runtime_keeps_layout_work_for_keyed_root_spacing_change() {
        let mut runtime = UiRuntime::new(&FallbackTextSystem);
        runtime.set_root(
            column(vec![text("Hello").key("title"), text("World").key("body")])
                .spacing(4.0)
                .key("root"),
        );
        runtime.resize(Size::new(320.0, 240.0));

        let first = runtime.prepare_frame().expect("frame").expect("scene");
        runtime.set_root(
            column(vec![text("Hello").key("title"), text("World").key("body")])
                .spacing(12.0)
                .key("root"),
        );
        let second = runtime.prepare_frame().expect("frame").expect("scene");

        assert_eq!(second.compose_stats.layout_passes, first.compose_stats.layout_passes + 1);
        assert!(matches!(second.scene_submit, SceneSubmit::Patch { .. }));
    }
}

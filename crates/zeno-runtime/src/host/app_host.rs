use zeno_core::{AppConfig, Platform, ZenoError};
use zeno_platform::desktop::DesktopShell;
use zeno_platform::presenter::{
    AnimatedFrameContext, AnimatedFrameOutput, FrameRequest, ResolvedWindowRun,
};
use zeno_scene::{DisplayList, FrameReport, RenderSession, RetainedScene};
use zeno_text::{FallbackTextSystem, TextSystem};

use crate::{App, AppFrame, AppView, PointerState, UiRuntime};
use super::UiSceneUpdate;

pub struct AppHost<A> {
    app: A,
    runtime: UiRuntime<'static>,
    platform: Platform,
}

impl<A> AppHost<A>
where
    A: App,
{
    #[must_use]
    pub fn new(app: A, text_system: &'static dyn TextSystem, platform: Platform) -> Self {
        Self {
            app,
            runtime: UiRuntime::new(text_system),
            platform,
        }
    }

    pub fn frame(
        &mut self,
        raw: AnimatedFrameContext,
        session: &mut dyn RenderSession,
    ) -> Result<AnimatedFrameOutput, ZenoError> {
        let last_report = raw.last_report.clone();
        let frame = AppFrame {
            frame_index: raw.frame_index,
            elapsed: raw.elapsed,
            delta: raw.delta,
            size: raw.size,
            platform: self.platform,
            backend: raw.backend,
            last_report,
            pointer: PointerState {
                position: raw.pointer.position,
                pressed: raw.pointer.pressed,
                just_pressed: raw.pointer.just_pressed,
                just_released: raw.pointer.just_released,
            },
        };
        let view = self.app.render(&frame);
        let report = match view {
            AppView::Compose(root) => {
                self.runtime.resize(frame.size);
                self.runtime.set_root(root);
                if let Some(ui_frame) = self.runtime.prepare_frame()? {
                    let supports_display_list_submit = session.capabilities().display_list_submit;
                    match ui_frame.scene_update {
                        UiSceneUpdate::Full { scene, display_list, .. } => {
                            let mut report = if supports_display_list_submit {
                                session.submit_display_list(&display_list, None, 0, 0)?
                            } else {
                                session.submit_retained_scene(scene, None, 0, 0)?
                            };
                            apply_display_list_stats(&mut report, &display_list);
                            report
                        }
                        UiSceneUpdate::Delta {
                            scene,
                            delta,
                            dirty_bounds,
                            display_list,
                            ..
                        } => {
                            let patch_upserts = delta.object_upserts.len() + delta.layer_upserts.len();
                            let patch_removes = delta.object_removes.len() + delta.layer_removes.len();
                            let mut report = if supports_display_list_submit {
                                session.submit_display_list(
                                    &display_list,
                                    dirty_bounds,
                                    patch_upserts,
                                    patch_removes,
                                )?
                            } else {
                                session.submit_retained_scene(
                                    scene,
                                    dirty_bounds,
                                    patch_upserts,
                                    patch_removes,
                                )?
                            };
                            apply_display_list_stats(&mut report, &display_list);
                            report
                        }
                    }
                } else {
                    frame.last_report.clone().ok_or_else(|| {
                        ZenoError::invalid_configuration(
                            zeno_core::ZenoErrorCode::UiRuntimeRootNotSet,
                            "runtime.app",
                            "frame",
                            "compose frame produced no update and no previous report exists",
                        )
                    })?
                }
            }
            AppView::Scene(scene) => {
                let mut retained = RetainedScene::from_scene(scene);
                session.submit_retained_scene(&mut retained, None, 0, 0)?
            }
        };
        let frame_request = match self.app.animation_interval(&frame) {
            None => FrameRequest::Wait,
            Some(duration) if duration.is_zero() => FrameRequest::NextFrame,
            Some(duration) => FrameRequest::After(duration),
        };
        Ok(AnimatedFrameOutput::submitted(report, frame_request))
    }
}

fn apply_display_list_stats(report: &mut FrameReport, display_list: &DisplayList) {
    report.display_item_count = display_list.items.len();
    report.stacking_context_count = display_list.stacking_contexts.len();
}

#[cfg(feature = "desktop_winit")]
pub fn run_app<A>(app_config: &AppConfig, app: A) -> Result<ResolvedWindowRun, ZenoError>
where
    A: App + 'static,
{
    run_app_with_text_system(app_config, Box::leak(Box::new(FallbackTextSystem)), app)
}

#[cfg(not(feature = "desktop_winit"))]
pub fn run_app<A>(_app_config: &AppConfig, _app: A) -> Result<ResolvedWindowRun, ZenoError>
where
    A: App + 'static,
{
    Err(ZenoError::invalid_configuration(
        zeno_core::ZenoErrorCode::WindowFeatureDisabled,
        "runtime.app",
        "run_app",
        "desktop app host requires desktop_winit",
    ))
}

#[cfg(feature = "desktop_winit")]
pub fn run_app_with_text_system<A>(
    app_config: &AppConfig,
    text_system: &'static dyn TextSystem,
    app: A,
) -> Result<ResolvedWindowRun, ZenoError>
where
    A: App + 'static,
{
    let session = DesktopShell.prepare_app_window_session(app_config)?;
    let outcome = ResolvedWindowRun {
        backend: session.backend.backend_kind,
        attempts: session.backend.attempts.clone(),
    };
    let mut host = AppHost::new(app, text_system, session.platform);
    DesktopShell.run_animated_scene_window(session, move |context, session| {
        host.frame(context, session)
    })?;
    Ok(outcome)
}

#[cfg(not(feature = "desktop_winit"))]
pub fn run_app_with_text_system<A>(
    _app_config: &AppConfig,
    _text_system: &'static dyn TextSystem,
    _app: A,
) -> Result<ResolvedWindowRun, ZenoError>
where
    A: App + 'static,
{
    Err(ZenoError::invalid_configuration(
        zeno_core::ZenoErrorCode::WindowFeatureDisabled,
        "runtime.app",
        "run_app_with_text_system",
        "desktop app host requires desktop_winit",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    struct StaticApp;

    impl App for StaticApp {
        fn render(&mut self, _frame: &AppFrame) -> AppView {
            AppView::Scene(zeno_scene::Scene::new(zeno_core::Size::new(1.0, 1.0)))
        }

        fn animation_interval(&self, _frame: &AppFrame) -> Option<Duration> {
            Some(Duration::from_millis(16))
        }
    }

    #[test]
    fn app_host_builds_frame_request_from_animation_interval() {
        let mut host = AppHost::new(StaticApp, &FallbackTextSystem, Platform::Linux);
        let output = host
            .frame(
                AnimatedFrameContext {
            frame_index: 0,
            elapsed: Duration::from_millis(16),
            delta: Duration::from_millis(16),
            size: zeno_core::Size::new(320.0, 240.0),
            backend: zeno_core::Backend::Skia,
            last_report: None,
            pointer: zeno_platform::event::PointerState::default(),
                },
                &mut DummyRenderSession,
            )
            .expect("frame");
        assert!(matches!(output.frame_request, FrameRequest::After(_)));
    }

    struct DummyRenderSession;

    impl zeno_scene::RenderSession for DummyRenderSession {
        fn kind(&self) -> zeno_core::Backend { zeno_core::Backend::Skia }
        fn capabilities(&self) -> zeno_scene::RenderCapabilities {
            zeno_scene::RenderCapabilities {
                gpu_compositing: true,
                text_shaping: true,
                filters: true,
                offscreen_rendering: true,
                display_list_submit: true,
            }
        }
        fn surface(&self) -> &zeno_scene::RenderSurface { panic!("unused") }
        fn resize(&mut self, _width: u32, _height: u32) -> Result<(), ZenoError> { Ok(()) }
        fn submit_retained_scene(
            &mut self,
            _scene: &mut zeno_scene::RetainedScene,
            _dirty_bounds: Option<zeno_core::Rect>,
            _patch_upserts: usize,
            _patch_removes: usize,
        ) -> Result<zeno_scene::FrameReport, ZenoError> {
            Ok(zeno_scene::FrameReport {
                backend: zeno_core::Backend::Skia,
                command_count: 0,
                resource_count: 0,
                block_count: 0,
                display_item_count: 0,
                stacking_context_count: 0,
                patch_upserts: 0,
                patch_removes: 0,
                surface_id: "dummy".into(),
            })
        }
        fn submit_display_list(
            &mut self,
            display_list: &zeno_scene::DisplayList,
            _dirty_bounds: Option<zeno_core::Rect>,
            _patch_upserts: usize,
            _patch_removes: usize,
        ) -> Result<zeno_scene::FrameReport, ZenoError> {
            Ok(zeno_scene::FrameReport {
                backend: zeno_core::Backend::Skia,
                command_count: display_list.items.len(),
                resource_count: 0,
                block_count: 0,
                display_item_count: display_list.items.len(),
                stacking_context_count: display_list.stacking_contexts.len(),
                patch_upserts: 0,
                patch_removes: 0,
                surface_id: "dummy".into(),
            })
        }
    }
}

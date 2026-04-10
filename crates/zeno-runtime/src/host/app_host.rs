use zeno_core::{AppConfig, Platform, ZenoError};
use zeno_platform::desktop::DesktopShell;
use zeno_platform::presenter::{
    AnimatedFrameContext, AnimatedFrameOutput, FrameRequest, ResolvedWindowRun,
};
use zeno_scene::RenderSceneUpdate;
use zeno_text::{FallbackTextSystem, TextSystem};

use crate::{App, AppFrame, AppView, PointerState, UiRuntime};

pub struct AppHost<A> {
    app: A,
    runtime: UiRuntime<'static>,
    last_compose_update: Option<RenderSceneUpdate>,
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
            last_compose_update: None,
            platform,
        }
    }

    pub fn frame(&mut self, raw: AnimatedFrameContext) -> AnimatedFrameOutput {
        let frame = AppFrame {
            frame_index: raw.frame_index,
            elapsed: raw.elapsed,
            delta: raw.delta,
            size: raw.size,
            platform: self.platform,
            backend: raw.backend,
            last_report: raw.last_report,
            pointer: PointerState {
                position: raw.pointer.position,
                pressed: raw.pointer.pressed,
                just_pressed: raw.pointer.just_pressed,
                just_released: raw.pointer.just_released,
            },
        };
        let view = self.app.render(&frame);
        let scene_update = match view {
            AppView::Compose(root) => {
                self.runtime.resize(frame.size);
                self.runtime.set_root(root);
                if let Some(ui_frame) = self.runtime.prepare_frame().expect("app host frame") {
                    self.last_compose_update = Some(ui_frame.scene_update.clone());
                    ui_frame.scene_update
                } else {
                    self.last_compose_update
                        .clone()
                        .expect("compose app view should have previous update")
                }
            }
            AppView::Scene(scene) => RenderSceneUpdate::Full(scene),
        };
        let frame_request = match self.app.animation_interval(&frame) {
            None => FrameRequest::Wait,
            Some(duration) if duration.is_zero() => FrameRequest::NextFrame,
            Some(duration) => FrameRequest::After(duration),
        };
        AnimatedFrameOutput {
            scene_update,
            frame_request,
        }
    }
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
    DesktopShell.run_animated_scene_window(session, move |context| host.frame(context))?;
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
        let output = host.frame(AnimatedFrameContext {
            frame_index: 0,
            elapsed: Duration::from_millis(16),
            delta: Duration::from_millis(16),
            size: zeno_core::Size::new(320.0, 240.0),
            backend: zeno_core::Backend::Skia,
            last_report: None,
            pointer: zeno_platform::event::PointerState::default(),
        });
        assert!(matches!(output.frame_request, FrameRequest::After(_)));
    }
}

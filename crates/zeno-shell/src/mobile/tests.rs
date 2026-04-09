use std::num::NonZeroUsize;

use zeno_core::{AppConfig, Backend, Color, Platform, RendererConfig, Size, WindowConfig, ZenoErrorCode};
use zeno_graphics::{DrawCommand, Scene, SceneSubmit};
use zeno_runtime::{BackendAttempt, ResolvedBackend, ResolvedSession};

use super::{
    create_mobile_render_session, AndroidAttachContext, IosMetalLayerAttachContext,
    IosViewAttachContext, MobileAttachContext, MobileHostKind, MobilePlatform,
    MobilePresenterAttachment, MobilePresenterInterface, MobilePresenterKind, MobileShell,
    MobileViewport,
};

fn fake_handle(seed: usize) -> NonZeroUsize {
    NonZeroUsize::new(seed).expect("non-zero handle")
}

fn test_submit() -> SceneSubmit {
    SceneSubmit::Full(Scene {
        size: Size::new(120.0, 80.0),
        clear_color: Some(Color::WHITE),
        commands: vec![DrawCommand::Clear(Color::WHITE)],
        layers: vec![zeno_graphics::SceneLayer::root(Size::new(120.0, 80.0))],
        blocks: Vec::new(),
    })
}

#[test]
fn mobile_shell_uses_requested_platform_descriptor() {
    let shell = MobileShell::android();
    let surface = shell.create_mobile_surface(&WindowConfig::default(), None);

    assert_eq!(surface.descriptor.platform, zeno_core::Platform::Android);
    assert_eq!(surface.surface.platform, zeno_core::Platform::Android);
}

#[test]
fn bind_session_applies_viewport_size_and_scale() {
    let shell = MobileShell::ios();
    let session = ResolvedSession::new(
        zeno_core::Platform::Ios,
        WindowConfig::default(),
        ResolvedBackend {
            backend_kind: Backend::Skia,
            attempts: vec![BackendAttempt {
                backend: Backend::Skia,
                reason: None,
            }],
        },
        false,
    );
    let binding = shell
        .bind_session(
            session,
            MobileViewport {
                width: 390.0,
                height: 844.0,
                scale_factor: 3.0,
            },
        )
        .expect("mobile session binding");

    assert_eq!(binding.backend, Backend::Skia);
    assert_eq!(binding.presenter, MobilePresenterKind::SkiaSurface);
    assert_eq!(binding.session.window.size.width, 390.0);
    assert_eq!(binding.session.window.size.height, 844.0);
    assert_eq!(binding.session.window.scale_factor, 3.0);
    assert_eq!(binding.surface.surface.size.width, 390.0);
    assert_eq!(binding.surface.surface.size.height, 844.0);
    assert_eq!(binding.surface.surface.scale_factor, 3.0);
    assert_eq!(binding.surface.surface.platform, zeno_core::Platform::Ios);
}

#[test]
fn bind_session_rejects_platform_mismatch() {
    let shell = MobileShell::android();
    let session = ResolvedSession::new(
        Platform::Ios,
        WindowConfig::default(),
        ResolvedBackend {
            backend_kind: Backend::Skia,
            attempts: vec![BackendAttempt {
                backend: Backend::Skia,
                reason: None,
            }],
        },
        false,
    );
    let error = shell
        .bind_session(
            session,
            MobileViewport {
                width: 390.0,
                height: 844.0,
                scale_factor: 3.0,
            },
        )
        .expect_err("platform mismatch should fail");

    assert_eq!(error.error_code(), ZenoErrorCode::MobileSessionPlatformMismatch);
}

#[test]
fn prepare_app_session_resolves_backend_before_binding() {
    let shell = MobileShell::android();
    let binding = shell
        .prepare_app_session(
            &AppConfig {
                renderer: RendererConfig::default(),
                ..AppConfig::default()
            },
            MobileViewport {
                width: 412.0,
                height: 915.0,
                scale_factor: 2.75,
            },
        )
        .expect("android session binding");

    assert_eq!(binding.platform, MobilePlatform::Android);
    assert_eq!(binding.session.platform, Platform::Android);
    assert_eq!(binding.backend, Backend::Skia);
    assert_eq!(binding.presenter, MobilePresenterKind::SkiaSurface);
    assert_eq!(binding.session.window.size.width, 412.0);
    assert_eq!(binding.session.window.size.height, 915.0);
    assert_eq!(binding.surface.surface.platform, Platform::Android);
}

#[test]
fn attach_session_accepts_android_native_window() {
    let shell = MobileShell::android();
    let attached = shell
        .prepare_attached_app_session(
            &AppConfig::default(),
            MobileViewport {
                width: 412.0,
                height: 915.0,
                scale_factor: 2.75,
            },
            MobileAttachContext::AndroidSurface(AndroidAttachContext {
                native_window: fake_handle(1),
            }),
        )
        .expect("android attached session");

    assert_eq!(attached.binding.platform, MobilePlatform::Android);
    assert_eq!(
        attached.attachment,
        MobilePresenterAttachment {
            host_kind: MobileHostKind::AndroidNativeWindow,
            presenter: MobilePresenterKind::SkiaSurface,
            interface: MobilePresenterInterface::AndroidSkiaNativeWindow,
        }
    );
}

#[test]
fn attach_session_rejects_attach_platform_mismatch() {
    let shell = MobileShell::android();
    let binding = shell
        .prepare_app_session(
            &AppConfig::default(),
            MobileViewport {
                width: 412.0,
                height: 915.0,
                scale_factor: 2.75,
            },
        )
        .expect("android binding");
    let error = shell
        .attach_session(
            binding,
            MobileAttachContext::IosView(IosViewAttachContext {
                ui_view: fake_handle(2),
            }),
        )
        .expect_err("platform mismatch should fail");

    assert_eq!(error.error_code(), ZenoErrorCode::MobileAttachPlatformMismatch);
}

#[test]
fn attach_session_rejects_impeller_without_required_host() {
    let shell = MobileShell::ios();
    let binding = shell
        .bind_session(
            ResolvedSession::new(
                Platform::Ios,
                WindowConfig::default(),
                ResolvedBackend {
                    backend_kind: Backend::Impeller,
                    attempts: vec![BackendAttempt {
                        backend: Backend::Impeller,
                        reason: None,
                    }],
                },
                false,
            ),
            MobileViewport {
                width: 390.0,
                height: 844.0,
                scale_factor: 3.0,
            },
        )
        .expect("ios impeller binding");
    let error = shell
        .attach_session(
            binding,
            MobileAttachContext::IosView(IosViewAttachContext {
                ui_view: fake_handle(3),
            }),
        )
        .expect_err("impeller should require a metal layer host");

    assert_eq!(error.error_code(), ZenoErrorCode::BackendMissingPlatformSurface);
}

#[test]
fn attach_session_accepts_ios_metal_layer_for_impeller() {
    let shell = MobileShell::ios();
    let binding = shell
        .bind_session(
            ResolvedSession::new(
                Platform::Ios,
                WindowConfig::default(),
                ResolvedBackend {
                    backend_kind: Backend::Impeller,
                    attempts: vec![BackendAttempt {
                        backend: Backend::Impeller,
                        reason: None,
                    }],
                },
                false,
            ),
            MobileViewport {
                width: 390.0,
                height: 844.0,
                scale_factor: 3.0,
            },
        )
        .expect("ios impeller binding");
    let attached = shell
        .attach_session(
            binding,
            MobileAttachContext::IosMetalLayer(IosMetalLayerAttachContext {
                metal_layer: fake_handle(4),
                ui_view: Some(fake_handle(5)),
            }),
        )
        .expect("impeller metal attachment");

    assert_eq!(attached.attachment.host_kind, MobileHostKind::IosMetalLayer);
    assert_eq!(attached.attachment.presenter, MobilePresenterKind::ImpellerSurface);
    assert_eq!(
        attached.attachment.interface,
        MobilePresenterInterface::IosImpellerMetalLayer
    );
}

#[test]
fn create_render_session_builds_android_session() {
    let shell = MobileShell::android();
    let attached = shell
        .prepare_attached_app_session(
            &AppConfig::default(),
            MobileViewport {
                width: 412.0,
                height: 915.0,
                scale_factor: 2.75,
            },
            MobileAttachContext::AndroidSurface(AndroidAttachContext {
                native_window: fake_handle(6),
            }),
        )
        .expect("android attached session");
    let mut session = create_mobile_render_session(attached).expect("android render session");
    let report = session.submit_scene(&test_submit()).expect("submit scene");

    assert_eq!(session.kind(), Backend::Skia);
    assert_eq!(session.attachment().host_kind, MobileHostKind::AndroidNativeWindow);
    assert_eq!(
        session.attachment().interface,
        MobilePresenterInterface::AndroidSkiaNativeWindow
    );
    assert_eq!(report.backend, Backend::Skia);
    assert_eq!(report.command_count, 1);
    assert_eq!(report.surface_id, "android-surface");
}

#[test]
fn prepare_render_session_builds_android_skia_session() {
    let shell = MobileShell::android();
    let mut session = shell
        .prepare_render_session(
            &AppConfig::default(),
            MobileViewport {
                width: 412.0,
                height: 915.0,
                scale_factor: 2.75,
            },
            MobileAttachContext::AndroidSurface(AndroidAttachContext {
                native_window: fake_handle(7),
            }),
        )
        .expect("android render session");
    session.resize(800, 600).expect("resize mobile session");
    let report = session.submit_scene(&test_submit()).expect("submit scene");

    assert_eq!(session.kind(), Backend::Skia);
    assert_eq!(session.surface().size.width, 800.0);
    assert_eq!(session.surface().size.height, 600.0);
    assert_eq!(session.attachment().host_kind, MobileHostKind::AndroidNativeWindow);
    assert_eq!(
        session.attachment().interface,
        MobilePresenterInterface::AndroidSkiaNativeWindow
    );
    assert_eq!(report.backend, Backend::Skia);
}

#[test]
fn create_render_session_builds_ios_impeller_session() {
    let shell = MobileShell::ios();
    let attached = shell
        .attach_session(
            shell.bind_session(
                ResolvedSession::new(
                    Platform::Ios,
                    WindowConfig::default(),
                    ResolvedBackend {
                        backend_kind: Backend::Impeller,
                        attempts: vec![BackendAttempt {
                            backend: Backend::Impeller,
                            reason: None,
                        }],
                    },
                    false,
                ),
                MobileViewport {
                    width: 390.0,
                    height: 844.0,
                    scale_factor: 3.0,
                },
            )
            .expect("ios impeller binding"),
            MobileAttachContext::IosMetalLayer(IosMetalLayerAttachContext {
                metal_layer: fake_handle(8),
                ui_view: Some(fake_handle(9)),
            }),
        )
        .expect("ios attached session");
    let mut session = shell
        .create_render_session(attached)
        .expect("ios render session");
    session.resize(800, 600).expect("resize mobile session");
    let report = session.submit_scene(&test_submit()).expect("submit scene");

    assert_eq!(session.kind(), Backend::Impeller);
    assert_eq!(session.surface().size.width, 800.0);
    assert_eq!(session.surface().size.height, 600.0);
    assert_eq!(session.attachment().host_kind, MobileHostKind::IosMetalLayer);
    assert_eq!(
        session.attachment().interface,
        MobilePresenterInterface::IosImpellerMetalLayer
    );
    assert_eq!(report.backend, Backend::Impeller);
}

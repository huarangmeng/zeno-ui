use std::num::NonZeroUsize;

use crate::session::{BackendAttempt, ResolvedBackend, ResolvedSession};
use zeno_core::{
    AppConfig, Backend, Color, Platform, RendererConfig, Size, WindowConfig, ZenoErrorCode,
};
use zeno_scene::{
    ClipChainId, DisplayItem, DisplayItemId, DisplayItemPayload, DisplayList, Scene, SpatialNodeId,
};

use super::{
    AndroidAttachContext, IosMetalLayerAttachContext, IosViewAttachContext, MobileAttachContext,
    MobileHostKind, MobilePlatform, MobilePresenterAttachment, MobilePresenterInterface,
    MobilePresenterKind, MobileShell, MobileViewport, create_mobile_render_session,
};
use crate::{NativeSurfaceHostAttachment, NativeSurfaceHostRequirement};

fn fake_handle(seed: usize) -> NonZeroUsize {
    NonZeroUsize::new(seed).expect("non-zero handle")
}

fn test_scene() -> Scene {
    let mut scene = Scene::new(Size::new(120.0, 80.0));
    scene.clear_color = Some(Color::WHITE);
    scene
}

fn test_display_list() -> DisplayList {
    let size = Size::new(120.0, 80.0);
    let mut display_list = DisplayList::empty(size);
    display_list.items.push(DisplayItem {
        item_id: DisplayItemId(1),
        spatial_id: SpatialNodeId(0),
        clip_chain_id: ClipChainId(0),
        stacking_context: None,
        visual_rect: zeno_core::Rect::new(0.0, 0.0, size.width, size.height),
        payload: DisplayItemPayload::FillRect {
            rect: zeno_core::Rect::new(0.0, 0.0, size.width, size.height),
            color: Color::WHITE,
        },
    });
    display_list
}

#[test]
fn mobile_shell_uses_requested_platform_descriptor() {
    let shell = MobileShell::android();
    let surface = shell.create_mobile_surface(&WindowConfig::default(), None);

    assert_eq!(surface.descriptor.platform, zeno_core::Platform::Android);
    assert_eq!(surface.surface.platform, zeno_core::Platform::Android);
    assert_eq!(
        surface.host_requirement,
        NativeSurfaceHostRequirement::AndroidNativeWindow
    );
    assert_eq!(surface.host_attachment, NativeSurfaceHostAttachment::None);
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
    assert_eq!(
        binding.surface.host_requirement,
        NativeSurfaceHostRequirement::IosViewOrMetalLayer
    );
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

    assert_eq!(
        error.error_code(),
        ZenoErrorCode::MobileSessionPlatformMismatch
    );
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
    assert_eq!(
        binding.surface.host_requirement,
        NativeSurfaceHostRequirement::AndroidNativeWindow
    );
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
    assert_eq!(
        attached.binding.surface.host_attachment,
        NativeSurfaceHostAttachment::AndroidNativeWindow {
            native_window: fake_handle(1),
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

    assert_eq!(
        error.error_code(),
        ZenoErrorCode::MobileAttachPlatformMismatch
    );
}

#[test]
fn attach_session_rejects_impeller_without_required_host() {
    let shell = MobileShell::ios();
    let error = shell
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
        .expect_err("ios impeller binding should fail before attach");
    assert_eq!(
        error.error_code(),
        ZenoErrorCode::BackendNotImplementedForPlatform
    );
}

#[test]
fn bind_session_rejects_ios_impeller_even_with_metal_layer_capability() {
    let shell = MobileShell::ios();
    let error = shell
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
        .expect_err("ios impeller binding should fail");

    assert_eq!(
        error.error_code(),
        ZenoErrorCode::BackendNotImplementedForPlatform
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
    let display_list = test_display_list();
    let report = session
        .submit_compositor_frame(&zeno_scene::CompositorFrame::full(display_list, 0))
        .expect("submit display list");

    assert_eq!(session.kind(), Backend::Skia);
    assert_eq!(
        session.attachment().host_kind,
        MobileHostKind::AndroidNativeWindow
    );
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
    let display_list = test_display_list();
    let report = session
        .submit_compositor_frame(&zeno_scene::CompositorFrame::full(display_list, 0))
        .expect("submit display list");

    assert_eq!(session.kind(), Backend::Skia);
    assert_eq!(session.surface().size.width, 800.0);
    assert_eq!(session.surface().size.height, 600.0);
    assert_eq!(
        session.attachment().host_kind,
        MobileHostKind::AndroidNativeWindow
    );
    assert_eq!(
        session.attachment().interface,
        MobilePresenterInterface::AndroidSkiaNativeWindow
    );
    assert_eq!(report.backend, Backend::Skia);
}

#[test]
fn attach_session_accepts_ios_skia_metal_layer_and_carries_host_attachment() {
    let shell = MobileShell::ios();
    let attached = shell
        .attach_session(
            shell
                .bind_session(
                    ResolvedSession::new(
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
                    ),
                    MobileViewport {
                        width: 390.0,
                        height: 844.0,
                        scale_factor: 3.0,
                    },
                )
                .expect("ios skia binding"),
            MobileAttachContext::IosMetalLayer(IosMetalLayerAttachContext {
                metal_layer: fake_handle(8),
                ui_view: Some(fake_handle(9)),
            }),
        )
        .expect("ios attached session");

    assert_eq!(attached.attachment.host_kind, MobileHostKind::IosMetalLayer);
    assert_eq!(
        attached.attachment.interface,
        MobilePresenterInterface::IosSkiaMetalLayer
    );
    assert_eq!(
        attached.binding.surface.host_attachment,
        NativeSurfaceHostAttachment::IosMetalLayer {
            metal_layer: fake_handle(8),
            ui_view: Some(fake_handle(9)),
        }
    );
}

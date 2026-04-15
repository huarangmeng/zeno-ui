use std::num::NonZeroUsize;

use zeno_core::{Backend, BackendUnavailableReason, Platform, WindowConfig, ZenoError};
use zeno_scene::RenderSurface;

use crate::platform;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformDescriptor {
    pub platform: Platform,
    pub impeller_preferred: bool,
    pub notes: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSurface {
    pub surface: RenderSurface,
    pub descriptor: PlatformDescriptor,
    pub target_backend: Option<Backend>,
    pub host_requirement: NativeSurfaceHostRequirement,
    pub host_attachment: NativeSurfaceHostAttachment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSurfaceHostRequirement {
    None,
    DesktopWindow,
    AndroidNativeWindow,
    IosView,
    IosMetalLayer,
    IosViewOrMetalLayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSurfaceHostAttachment {
    None,
    AndroidNativeWindow {
        native_window: NonZeroUsize,
    },
    IosView {
        ui_view: NonZeroUsize,
    },
    IosMetalLayer {
        metal_layer: NonZeroUsize,
        ui_view: Option<NonZeroUsize>,
    },
}

impl NativeSurface {
    #[must_use]
    pub fn with_attachment(mut self, host_attachment: NativeSurfaceHostAttachment) -> Self {
        self.host_attachment = host_attachment;
        self
    }

    #[must_use]
    pub fn accepts_attachment(&self, host_attachment: NativeSurfaceHostAttachment) -> bool {
        matches!(
            (self.host_requirement, host_attachment),
            (
                NativeSurfaceHostRequirement::None,
                NativeSurfaceHostAttachment::None
            ) | (
                NativeSurfaceHostRequirement::AndroidNativeWindow,
                NativeSurfaceHostAttachment::AndroidNativeWindow { .. }
            ) | (
                NativeSurfaceHostRequirement::IosView,
                NativeSurfaceHostAttachment::IosView { .. }
            ) | (
                NativeSurfaceHostRequirement::IosMetalLayer,
                NativeSurfaceHostAttachment::IosMetalLayer { .. }
            ) | (
                NativeSurfaceHostRequirement::IosViewOrMetalLayer,
                NativeSurfaceHostAttachment::IosView { .. }
            ) | (
                NativeSurfaceHostRequirement::IosViewOrMetalLayer,
                NativeSurfaceHostAttachment::IosMetalLayer { .. }
            )
        )
    }
}

#[allow(dead_code)]
pub(crate) fn host_requirement_for_backend(
    platform: Platform,
    backend: Backend,
) -> Result<NativeSurfaceHostRequirement, ZenoError> {
    match (platform, backend) {
        (Platform::Windows | Platform::Linux | Platform::MacOs, Backend::Skia) => {
            Ok(NativeSurfaceHostRequirement::DesktopWindow)
        }
        (Platform::MacOs, Backend::Impeller) => Ok(NativeSurfaceHostRequirement::DesktopWindow),
        (Platform::Android, Backend::Skia) => Ok(NativeSurfaceHostRequirement::AndroidNativeWindow),
        (Platform::Ios, Backend::Skia) => Ok(NativeSurfaceHostRequirement::IosViewOrMetalLayer),
        (
            Platform::Windows | Platform::Linux | Platform::Android | Platform::Ios,
            Backend::Impeller,
        ) => Err(ZenoError::BackendUnavailable {
            backend,
            reason: BackendUnavailableReason::NotImplementedForPlatform,
        }),
        (Platform::Unknown, _) => Err(ZenoError::BackendUnavailable {
            backend,
            reason: BackendUnavailableReason::NotImplementedForPlatform,
        }),
    }
}

pub trait Shell {
    fn create_surface(&self, config: &WindowConfig) -> NativeSurface;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MinimalShell;

impl Shell for MinimalShell {
    fn create_surface(&self, config: &WindowConfig) -> NativeSurface {
        create_native_surface(config, None, None, NativeSurfaceHostRequirement::None)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DesktopShell;

impl Shell for DesktopShell {
    fn create_surface(&self, config: &WindowConfig) -> NativeSurface {
        create_native_surface(
            config,
            None,
            None,
            NativeSurfaceHostRequirement::DesktopWindow,
        )
    }
}

#[must_use]
pub fn current_platform_descriptor() -> PlatformDescriptor {
    match Platform::current() {
        Platform::Windows => platform::windows::descriptor(),
        Platform::MacOs => platform::macos::descriptor(),
        Platform::Linux => platform::linux::descriptor(),
        Platform::Android => platform::android::descriptor(),
        Platform::Ios => platform::ios::descriptor(),
        Platform::Unknown => PlatformDescriptor {
            platform: Platform::Unknown,
            impeller_preferred: false,
            notes: "unknown shell target",
        },
    }
}

pub(crate) fn create_native_surface(
    config: &WindowConfig,
    override_size: Option<(f32, f32)>,
    target_backend: Option<Backend>,
    host_requirement: NativeSurfaceHostRequirement,
) -> NativeSurface {
    let descriptor = current_platform_descriptor();
    let size = override_size
        .map(|(width, height)| zeno_core::Size::new(width, height))
        .unwrap_or(config.size);
    NativeSurface {
        surface: RenderSurface {
            id: format!("{}-surface", descriptor.platform),
            platform: descriptor.platform,
            size,
            scale_factor: config.scale_factor,
        },
        descriptor,
        target_backend,
        host_requirement,
        host_attachment: NativeSurfaceHostAttachment::None,
    }
}

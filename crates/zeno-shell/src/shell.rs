use zeno_core::{Platform, WindowConfig};
use zeno_graphics::RenderSurface;

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
}

pub trait Shell {
    fn create_surface(&self, config: &WindowConfig) -> NativeSurface;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MinimalShell;

impl Shell for MinimalShell {
    fn create_surface(&self, config: &WindowConfig) -> NativeSurface {
        create_native_surface(config, None)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DesktopShell;

impl Shell for DesktopShell {
    fn create_surface(&self, config: &WindowConfig) -> NativeSurface {
        MinimalShell.create_surface(config)
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
    }
}

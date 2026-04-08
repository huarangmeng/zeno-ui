use zeno_core::{Platform, WindowConfig, ZenoError, ZenoErrorCode};
use zeno_graphics::RenderSurface;
use zeno_runtime::ResolvedSession;

use crate::{
    platform,
    shell::{NativeSurface, PlatformDescriptor, Shell},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobilePlatform {
    Android,
    Ios,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MobileViewport {
    pub width: f32,
    pub height: f32,
    pub scale_factor: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MobileSessionBinding {
    pub platform: MobilePlatform,
    pub session: ResolvedSession,
    pub surface: NativeSurface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MobileShell {
    platform: MobilePlatform,
}

#[cfg(feature = "mobile_android")]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AndroidShell;

#[cfg(feature = "mobile_ios")]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IosShell;

impl MobileShell {
    #[cfg(feature = "mobile_android")]
    #[must_use]
    pub const fn android() -> Self {
        Self {
            platform: MobilePlatform::Android,
        }
    }

    #[cfg(feature = "mobile_ios")]
    #[must_use]
    pub const fn ios() -> Self {
        Self {
            platform: MobilePlatform::Ios,
        }
    }

    #[must_use]
    pub const fn platform(&self) -> MobilePlatform {
        self.platform
    }

    #[must_use]
    pub fn platform_descriptor(&self) -> PlatformDescriptor {
        descriptor_for(self.platform)
    }

    #[must_use]
    pub fn create_mobile_surface(
        &self,
        config: &WindowConfig,
        viewport: Option<MobileViewport>,
    ) -> NativeSurface {
        create_mobile_surface(self.platform, config, viewport)
    }

    pub fn bind_session(
        &self,
        session: ResolvedSession,
        viewport: MobileViewport,
    ) -> Result<MobileSessionBinding, ZenoError> {
        if viewport.width <= 0.0 || viewport.height <= 0.0 {
            return Err(ZenoError::invalid_configuration(
                ZenoErrorCode::MobileViewportInvalid,
                "shell.mobile",
                "bind_session",
                "mobile viewport must be positive",
            ));
        }

        let mut window = session.window.clone();
        window.scale_factor = viewport.scale_factor;
        let surface = self.create_mobile_surface(&window, Some(viewport));

        Ok(MobileSessionBinding {
            platform: self.platform,
            session,
            surface,
        })
    }
}

#[cfg(feature = "mobile_android")]
impl AndroidShell {
    #[must_use]
    pub const fn mobile() -> MobileShell {
        MobileShell::android()
    }

    pub fn bind_session(
        &self,
        session: ResolvedSession,
        viewport: MobileViewport,
    ) -> Result<MobileSessionBinding, ZenoError> {
        Self::mobile().bind_session(session, viewport)
    }
}

#[cfg(feature = "mobile_android")]
impl Shell for AndroidShell {
    fn create_surface(&self, config: &WindowConfig) -> NativeSurface {
        Self::mobile().create_mobile_surface(config, None)
    }
}

#[cfg(feature = "mobile_ios")]
impl IosShell {
    #[must_use]
    pub const fn mobile() -> MobileShell {
        MobileShell::ios()
    }

    pub fn bind_session(
        &self,
        session: ResolvedSession,
        viewport: MobileViewport,
    ) -> Result<MobileSessionBinding, ZenoError> {
        Self::mobile().bind_session(session, viewport)
    }
}

#[cfg(feature = "mobile_ios")]
impl Shell for IosShell {
    fn create_surface(&self, config: &WindowConfig) -> NativeSurface {
        Self::mobile().create_mobile_surface(config, None)
    }
}

fn descriptor_for(platform: MobilePlatform) -> PlatformDescriptor {
    match platform {
        MobilePlatform::Android => platform::android::descriptor(),
        MobilePlatform::Ios => platform::ios::descriptor(),
    }
}

fn create_mobile_surface(
    platform: MobilePlatform,
    config: &WindowConfig,
    viewport: Option<MobileViewport>,
) -> NativeSurface {
    let descriptor = descriptor_for(platform);
    let (width, height, scale_factor) = viewport
        .map(|viewport| (viewport.width, viewport.height, viewport.scale_factor))
        .unwrap_or((config.size.width, config.size.height, config.scale_factor));
    NativeSurface {
        surface: RenderSurface {
            id: format!("{}-surface", match platform {
                MobilePlatform::Android => Platform::Android,
                MobilePlatform::Ios => Platform::Ios,
            }),
            platform: match platform {
                MobilePlatform::Android => Platform::Android,
                MobilePlatform::Ios => Platform::Ios,
            },
            size: zeno_core::Size::new(width, height),
            scale_factor,
        },
        descriptor,
    }
}

#[cfg(test)]
mod tests {
    use super::{MobilePlatform, MobileShell, MobileViewport};
    use zeno_core::{Backend, WindowConfig};
    use zeno_runtime::{BackendAttempt, ResolvedBackend, ResolvedSession};

    #[test]
    fn mobile_shell_uses_requested_platform_descriptor() {
        let shell = MobileShell {
            platform: MobilePlatform::Android,
        };
        let surface = shell.create_mobile_surface(&WindowConfig::default(), None);

        assert_eq!(surface.descriptor.platform, zeno_core::Platform::Android);
        assert_eq!(surface.surface.platform, zeno_core::Platform::Android);
    }

    #[test]
    fn bind_session_applies_viewport_size_and_scale() {
        let shell = MobileShell {
            platform: MobilePlatform::Ios,
        };
        let session = ResolvedSession::new(
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

        assert_eq!(binding.surface.surface.size.width, 390.0);
        assert_eq!(binding.surface.surface.size.height, 844.0);
        assert_eq!(binding.surface.surface.scale_factor, 3.0);
        assert_eq!(binding.surface.surface.platform, zeno_core::Platform::Ios);
    }
}

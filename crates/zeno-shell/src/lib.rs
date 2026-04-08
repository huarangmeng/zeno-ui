use zeno_core::{PlatformKind, WindowConfig};
use zeno_graphics::RenderSurface;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformDescriptor {
    pub platform: PlatformKind,
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
        NativeSurface {
            surface: RenderSurface {
                id: format!("{}-surface", current_platform_descriptor().platform),
                platform: current_platform_descriptor().platform,
                size: config.size,
                scale_factor: config.scale_factor,
            },
            descriptor: current_platform_descriptor(),
        }
    }
}

pub mod platform {
    use super::PlatformDescriptor;
    use zeno_core::PlatformKind;

    pub mod windows {
        use super::{PlatformDescriptor, PlatformKind};

        #[must_use]
        pub fn descriptor() -> PlatformDescriptor {
            PlatformDescriptor {
                platform: PlatformKind::Windows,
                impeller_preferred: false,
                notes: "win32 shell with skia fallback first",
            }
        }
    }

    pub mod macos {
        use super::{PlatformDescriptor, PlatformKind};

        #[must_use]
        pub fn descriptor() -> PlatformDescriptor {
            PlatformDescriptor {
                platform: PlatformKind::MacOS,
                impeller_preferred: true,
                notes: "metal-backed nsview shell",
            }
        }
    }

    pub mod linux {
        use super::{PlatformDescriptor, PlatformKind};

        #[must_use]
        pub fn descriptor() -> PlatformDescriptor {
            PlatformDescriptor {
                platform: PlatformKind::Linux,
                impeller_preferred: false,
                notes: "wayland/x11 shell with skia default",
            }
        }
    }

    pub mod android {
        use super::{PlatformDescriptor, PlatformKind};

        #[must_use]
        pub fn descriptor() -> PlatformDescriptor {
            PlatformDescriptor {
                platform: PlatformKind::Android,
                impeller_preferred: true,
                notes: "android surface shell with native renderer handoff",
            }
        }
    }

    pub mod ios {
        use super::{PlatformDescriptor, PlatformKind};

        #[must_use]
        pub fn descriptor() -> PlatformDescriptor {
            PlatformDescriptor {
                platform: PlatformKind::IOS,
                impeller_preferred: true,
                notes: "uiview shell with metal layer",
            }
        }
    }
}

#[must_use]
pub fn current_platform_descriptor() -> PlatformDescriptor {
    match PlatformKind::current() {
        PlatformKind::Windows => platform::windows::descriptor(),
        PlatformKind::MacOS => platform::macos::descriptor(),
        PlatformKind::Linux => platform::linux::descriptor(),
        PlatformKind::Android => platform::android::descriptor(),
        PlatformKind::IOS => platform::ios::descriptor(),
        PlatformKind::Unknown => PlatformDescriptor {
            platform: PlatformKind::Unknown,
            impeller_preferred: false,
            notes: "unknown shell target",
        },
    }
}

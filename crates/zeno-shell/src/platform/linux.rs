use crate::PlatformDescriptor;
use zeno_core::Platform;

#[must_use]
pub fn descriptor() -> PlatformDescriptor {
    PlatformDescriptor {
        platform: Platform::Linux,
        impeller_preferred: false,
        notes: "wayland/x11 shell with skia default",
    }
}

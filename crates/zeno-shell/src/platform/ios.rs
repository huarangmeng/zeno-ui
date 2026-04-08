use crate::PlatformDescriptor;
use zeno_core::Platform;

#[must_use]
pub fn descriptor() -> PlatformDescriptor {
    PlatformDescriptor {
        platform: Platform::Ios,
        impeller_preferred: true,
        notes: "uiview shell with metal layer",
    }
}

use crate::PlatformDescriptor;
use zeno_core::Platform;

#[must_use]
pub fn descriptor() -> PlatformDescriptor {
    PlatformDescriptor {
        platform: Platform::MacOs,
        impeller_preferred: true,
        notes: "metal-backed nsview shell",
    }
}

use crate::PlatformDescriptor;
use zeno_core::Platform;

#[must_use]
pub fn descriptor() -> PlatformDescriptor {
    PlatformDescriptor {
        platform: Platform::Android,
        impeller_preferred: true,
        notes: "android surface shell with native renderer handoff",
    }
}

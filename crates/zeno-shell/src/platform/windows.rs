use crate::PlatformDescriptor;
use zeno_core::Platform;

#[must_use]
pub fn descriptor() -> PlatformDescriptor {
    PlatformDescriptor {
        platform: Platform::Windows,
        impeller_preferred: false,
        notes: "win32 shell with skia fallback first",
    }
}

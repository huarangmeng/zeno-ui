use std::sync::OnceLock;

use font_kit::source::SystemSource;
use fontdue::Font;
use rustybuzz::Face;

const SYSTEM_FONT_FAMILIES: [&str; 5] = [
    "PingFang SC",
    "Helvetica Neue",
    "Arial",
    "Noto Sans CJK SC",
    "Noto Sans",
];

#[must_use]
pub fn preferred_font_families(requested_family: &str) -> Vec<&str> {
    let mut families = Vec::with_capacity(SYSTEM_FONT_FAMILIES.len() + 1);
    if !requested_family.is_empty() && requested_family != "System" {
        families.push(requested_family);
    }
    for family in SYSTEM_FONT_FAMILIES {
        if !families.contains(&family) {
            families.push(family);
        }
    }
    families
}

#[must_use]
pub fn system_font_data() -> Option<&'static [u8]> {
    static FONT_DATA: OnceLock<Option<&'static [u8]>> = OnceLock::new();
    FONT_DATA.get_or_init(load_system_font_data).to_owned()
}

#[must_use]
pub fn system_font_available() -> bool {
    system_font_data().is_some()
}

#[must_use]
pub fn load_system_font() -> Option<Font> {
    let bytes = system_font_data()?;
    Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
}

#[must_use]
pub fn system_font_face() -> Option<&'static Face<'static>> {
    static FACE: OnceLock<Option<Face<'static>>> = OnceLock::new();
    FACE.get_or_init(|| system_font_data().and_then(|bytes| Face::from_slice(bytes, 0)))
        .as_ref()
}

fn load_system_font_data() -> Option<&'static [u8]> {
    for family in preferred_font_families("System") {
        if let Ok(handle) = SystemSource::new().select_family_by_name(family)
            && let Some(font_handle) = handle.fonts().first()
            && let Ok(font) = font_handle.load()
            && let Some(bytes) = font.copy_font_data()
        {
            let leaked: &'static mut [u8] = Box::leak(bytes.as_slice().to_vec().into_boxed_slice());
            return Some(leaked);
        }
    }
    None
}

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use font_kit::{
    family_name::FamilyName,
    handle::Handle,
    properties::{Properties, Stretch, Style, Weight},
    source::SystemSource,
};
use fontdue::Font;
use rustybuzz::Face;

use crate::FontDescriptor;

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
    system_font_data_for(&FontDescriptor::default())
}

#[must_use]
pub fn system_font_data_for(descriptor: &FontDescriptor) -> Option<&'static [u8]> {
    static FONT_DATA: OnceLock<Mutex<HashMap<FontDescriptor, Option<&'static [u8]>>>> =
        OnceLock::new();
    let cache = FONT_DATA.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(cached) = cache
        .lock()
        .expect("font data cache poisoned")
        .get(descriptor)
        .copied()
    {
        return cached;
    }
    let loaded = load_system_font_data_for(descriptor);
    cache
        .lock()
        .expect("font data cache poisoned")
        .insert(descriptor.clone(), loaded);
    loaded
}

#[must_use]
pub fn system_font_available() -> bool {
    system_font_data().is_some()
}

#[must_use]
pub fn load_system_font() -> Option<Font> {
    load_system_font_for(&FontDescriptor::default())
}

#[must_use]
pub fn load_system_font_for(descriptor: &FontDescriptor) -> Option<Font> {
    let bytes = system_font_data_for(descriptor)?;
    Font::from_bytes(bytes, fontdue::FontSettings::default()).ok()
}

#[must_use]
pub fn system_font_face_for(descriptor: &FontDescriptor) -> Option<Face<'static>> {
    let bytes = system_font_data_for(descriptor)?;
    Face::from_slice(bytes, 0)
}

fn load_system_font_data_for(descriptor: &FontDescriptor) -> Option<&'static [u8]> {
    let source = SystemSource::new();
    if let Some(bytes) = select_best_match_font_data(&source, descriptor) {
        return Some(bytes);
    }
    if descriptor.family != FontDescriptor::default().family {
        return load_system_font_data_for(&FontDescriptor::default());
    }
    None
}

fn select_best_match_font_data(
    source: &SystemSource,
    descriptor: &FontDescriptor,
) -> Option<&'static [u8]> {
    let family_names: Vec<_> = preferred_font_families(&descriptor.family)
        .into_iter()
        .map(|family| FamilyName::Title(family.to_owned()))
        .collect();
    let properties = font_properties(descriptor);
    if let Ok(handle) = source.select_best_match(&family_names, &properties)
        && let Some(bytes) = font_data_from_handle(handle)
    {
        return Some(bytes);
    }
    for family in preferred_font_families(&descriptor.family) {
        if let Ok(handle) = source.select_family_by_name(family)
            && let Some(font_handle) = handle.fonts().first()
            && let Some(bytes) = font_data_from_handle(font_handle.clone())
        {
            return Some(bytes);
        }
    }
    None
}

fn font_properties(descriptor: &FontDescriptor) -> Properties {
    Properties {
        style: if descriptor.italic {
            Style::Italic
        } else {
            Style::Normal
        },
        weight: Weight(descriptor.weight.0 as f32),
        stretch: Stretch::NORMAL,
    }
}

fn font_data_from_handle(handle: Handle) -> Option<&'static [u8]> {
    let font = handle.load().ok()?;
    let bytes = font.copy_font_data()?;
    let leaked: &'static mut [u8] = Box::leak(bytes.as_slice().to_vec().into_boxed_slice());
    Some(leaked)
}

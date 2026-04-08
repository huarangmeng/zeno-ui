use crate::{BackendPreference, Size};

pub type ScaleFactor = f32;

#[derive(Debug, Clone, PartialEq)]
pub struct WindowConfig {
    pub title: String,
    pub size: Size,
    pub scale_factor: ScaleFactor,
    pub transparent: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Zeno UI".to_string(),
            size: Size::new(1280.0, 720.0),
            scale_factor: 1.0,
            transparent: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererConfig {
    pub preference: BackendPreference,
    pub allow_fallback: bool,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            preference: BackendPreference::PreferImpeller,
            allow_fallback: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppConfig {
    pub app_name: String,
    pub renderer: RendererConfig,
    pub window: WindowConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app_name: "zeno-ui".to_string(),
            renderer: RendererConfig::default(),
            window: WindowConfig::default(),
        }
    }
}

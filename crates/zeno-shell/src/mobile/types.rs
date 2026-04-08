use std::num::NonZeroUsize;

use zeno_core::{Backend, Platform};
use zeno_graphics::RenderSession;
use zeno_runtime::ResolvedSession;

use crate::shell::NativeSurface;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobilePlatform {
    Android,
    Ios,
}

impl MobilePlatform {
    #[must_use]
    pub const fn as_platform(self) -> Platform {
        match self {
            Self::Android => Platform::Android,
            Self::Ios => Platform::Ios,
        }
    }
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
    pub backend: Backend,
    pub presenter: MobilePresenterKind,
    pub session: ResolvedSession,
    pub surface: NativeSurface,
}

impl MobileSessionBinding {
    #[must_use]
    pub fn surface_id(&self) -> &str {
        &self.surface.surface.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobileHostKind {
    AndroidNativeWindow,
    IosView,
    IosMetalLayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobilePresenterInterface {
    AndroidSkiaNativeWindow,
    AndroidImpellerNativeWindow,
    IosSkiaView,
    IosSkiaMetalLayer,
    IosImpellerMetalLayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AndroidAttachContext {
    pub native_window: NonZeroUsize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IosViewAttachContext {
    pub ui_view: NonZeroUsize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IosMetalLayerAttachContext {
    pub metal_layer: NonZeroUsize,
    pub ui_view: Option<NonZeroUsize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobileAttachContext {
    AndroidSurface(AndroidAttachContext),
    IosView(IosViewAttachContext),
    IosMetalLayer(IosMetalLayerAttachContext),
}

impl MobileAttachContext {
    #[must_use]
    pub const fn platform(self) -> Platform {
        match self {
            Self::AndroidSurface(_) => Platform::Android,
            Self::IosView(_) | Self::IosMetalLayer(_) => Platform::Ios,
        }
    }

    #[must_use]
    pub const fn host_kind(self) -> MobileHostKind {
        match self {
            Self::AndroidSurface(_) => MobileHostKind::AndroidNativeWindow,
            Self::IosView(_) => MobileHostKind::IosView,
            Self::IosMetalLayer(_) => MobileHostKind::IosMetalLayer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MobilePresenterAttachment {
    pub host_kind: MobileHostKind,
    pub presenter: MobilePresenterKind,
    pub interface: MobilePresenterInterface,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MobileAttachedSession {
    pub binding: MobileSessionBinding,
    pub attachment: MobilePresenterAttachment,
    pub context: MobileAttachContext,
}

impl MobileAttachedSession {
    #[must_use]
    pub fn surface_id(&self) -> &str {
        self.binding.surface_id()
    }
}

pub trait MobileRenderSessionHandle: RenderSession {
    fn attachment(&self) -> &MobilePresenterAttachment;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobilePresenterKind {
    SkiaSurface,
    ImpellerSurface,
}

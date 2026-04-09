//! 测试按主题拆分，降低 lib.rs 的体量与冲突面。

mod engine;
mod layers;
mod modifiers;
mod smoke;

use crate::{
    BlendMode, ComposeEngine, DirtyReason, EdgeInsets, Modifier, compose_scene, dump_layout,
    dump_scene,
};
use zeno_foundation::{column, container, row, spacer, text};
use zeno_core::{Color, Size, Transform2D};
use zeno_scene::{DrawCommand, Scene, SceneBlendMode, SceneClip, SceneEffect, SceneSubmit};
use zeno_text::FallbackTextSystem;

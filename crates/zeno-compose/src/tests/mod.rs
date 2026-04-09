//! 测试按主题拆分，降低 lib.rs 的体量与冲突面。

mod engine;
mod layers;
mod modifiers;
mod smoke;

use crate::{
    BlendMode, ComposeEngine, DirtyReason, EdgeInsets, Modifier, column, compose_scene, container,
    dump_layout, dump_scene, row, spacer, text,
};
use zeno_core::{Color, Size, Transform2D};
use zeno_graphics::{DrawCommand, Scene, SceneBlendMode, SceneClip, SceneEffect, SceneSubmit};
use zeno_text::FallbackTextSystem;

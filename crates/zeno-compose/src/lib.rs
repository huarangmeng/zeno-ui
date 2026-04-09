mod invalidation;
mod layout;
mod modifier;
mod node;
mod render;
mod style;
mod tree;
mod widgets;

pub use invalidation::{DirtyFlags, DirtyReason};
pub use modifier::{BlendMode, ClipMode, DropShadow, Modifier, Modifiers, TransformOrigin};
pub use node::NodeId;
pub use node::{Node, NodeKind, SpacerNode, TextNode};
pub use render::{
    ComposeEngine, ComposeRenderer, ComposeStats, compose_scene, dump_layout, dump_scene,
};
pub use style::{Axis, EdgeInsets, Style};
pub use widgets::{column, container, row, spacer, text};

#[cfg(test)]
mod tests;

//! 测试按主题拆分，降低 lib.rs 的体量与冲突面。

mod engine;
mod layers;
mod modifiers;
mod smoke;

use crate::{
    Alignment, Arrangement, BlendMode, ComposeEngine, CrossAxisAlignment, DirtyReason,
    EdgeInsets, Modifier, Node, NodeId, NodeKind, SpacerNode, TextNode, compose_scene,
    dump_layout, dump_scene,
};
use zeno_core::{Color, Size, Transform2D};
use zeno_scene::{DrawCommand, Scene, SceneBlendMode, SceneClip, SceneEffect, SceneSubmit};
use zeno_text::FallbackTextSystem;

// Local test helpers to avoid depending on zeno-foundation in this crate's tests,
// preventing duplicate zeno-ui crate instances in the dev dependency graph.
mod helpers {
    use std::sync::atomic::{AtomicU64, Ordering};

    use zeno_text::FontDescriptor;

    use super::{Node, NodeId, NodeKind, SpacerNode, TextNode};
    use crate::style::Axis;

    static NEXT_NODE_ID: AtomicU64 = AtomicU64::new(1);

    fn next_node_id() -> NodeId {
        NodeId(NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed))
    }

    pub fn text(content: impl Into<String>) -> Node {
        Node::new(
            next_node_id(),
            NodeKind::Text(TextNode {
                content: content.into(),
                font: FontDescriptor::default(),
                font_size: 16.0,
            }),
        )
    }

    pub fn spacer(width: f32, height: f32) -> Node {
        Node::new(
            next_node_id(),
            NodeKind::Spacer(SpacerNode { width, height }),
        )
    }

    pub fn container(child: Node) -> Node {
        Node::new(next_node_id(), NodeKind::Container(Box::new(child)))
    }

    pub fn r#box(children: Vec<Node>) -> Node {
        Node::new(next_node_id(), NodeKind::Box { children })
    }

    pub fn column(children: Vec<Node>) -> Node {
        Node::new(
            next_node_id(),
            NodeKind::Stack {
                axis: Axis::Vertical,
                children,
            },
        )
    }

    pub fn row(children: Vec<Node>) -> Node {
        Node::new(
            next_node_id(),
            NodeKind::Stack {
                axis: Axis::Horizontal,
                children,
            },
        )
    }
}

pub(crate) use helpers::*;

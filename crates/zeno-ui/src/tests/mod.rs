//! 测试按主题拆分，降低 lib.rs 的体量与冲突面。

mod engine;
mod layers;
mod modifiers;
mod smoke;

use crate::{
    Alignment, Arrangement, BlendMode, ComposeEngine, ComposeRenderer, CrossAxisAlignment, DirtyReason,
    EdgeInsets, Modifier, Node, NodeId, NodeKind, SpacerNode, TextNode, compose_scene,
    dump_layout, dump_scene, RetainedComposeUpdate,
};
use zeno_core::{Color, Point, Size, Transform2D};
use zeno_scene::{
    DisplayList, RenderSceneUpdate, Scene, SceneBlendMode, SceneClip, SceneEffect,
};
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

pub(crate) fn snapshot_submit(update: RetainedComposeUpdate<'_>) -> RenderSceneUpdate {
    match update {
        RetainedComposeUpdate::Full { scene, .. } => RenderSceneUpdate::Full(scene.snapshot_scene()),
        RetainedComposeUpdate::Delta { scene, delta, .. } => RenderSceneUpdate::Delta {
            current: scene.snapshot_scene(),
            delta,
        },
    }
}

pub(crate) fn snapshot_display_list(update: RetainedComposeUpdate<'_>) -> DisplayList {
    match update {
        RetainedComposeUpdate::Full { display_list, .. } => display_list,
        RetainedComposeUpdate::Delta { display_list, .. } => display_list,
    }
}

pub(crate) fn snapshot_outputs(update: RetainedComposeUpdate<'_>) -> (RenderSceneUpdate, DisplayList) {
    match update {
        RetainedComposeUpdate::Full {
            scene,
            display_list,
            ..
        } => {
            (RenderSceneUpdate::Full(scene.snapshot_scene()), display_list)
        }
        RetainedComposeUpdate::Delta {
            scene,
            delta,
            display_list,
            ..
        } => (
            RenderSceneUpdate::Delta {
                current: scene.snapshot_scene(),
                delta,
            },
            display_list,
        ),
    }
}

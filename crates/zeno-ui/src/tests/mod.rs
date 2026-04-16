//! 测试按主题拆分，降低 lib.rs 的体量与冲突面。

mod engine;
mod layers;
mod modifiers;
mod smoke;

use crate::{
    Alignment, Arrangement, BlendMode, ComposeEngine, ComposeRenderer, ComposeUpdate,
    CrossAxisAlignment, DirtyReason, EdgeInsets, FontFeature, FontFeatures, FontWeight,
    ImageNode, Modifier, Node, NodeId, NodeKind, SpacerNode, TextNode, TextStyle, dump_layout,
};
use zeno_core::{Color, Point, Size, Transform2D};
use zeno_scene::{ClipRegion, DisplayList, Effect};
use zeno_text::FallbackTextSystem;

// Local test helpers to avoid depending on zeno-foundation in this crate's tests,
// preventing duplicate zeno-ui crate instances in the dev dependency graph.
mod helpers {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{ImageNode, Node, NodeId, NodeKind, SpacerNode, TextNode};
    use crate::ImageSource;
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
            }),
        )
    }

    pub fn spacer(width: f32, height: f32) -> Node {
        Node::new(
            next_node_id(),
            NodeKind::Spacer(SpacerNode { width, height }),
        )
    }

    pub fn image_rgba8(width: f32, height: f32, rgba8: Vec<u8>) -> Node {
        Node::new(
            next_node_id(),
            NodeKind::Image(ImageNode {
                source: ImageSource::rgba8(
                    width.max(1.0).round() as u32,
                    height.max(1.0).round() as u32,
                    rgba8,
                ),
            }),
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

pub(crate) fn snapshot_display_list(update: ComposeUpdate) -> DisplayList {
    match update {
        ComposeUpdate::Full { display_list, .. } => display_list,
        ComposeUpdate::Delta { display_list, .. } => display_list,
    }
}

pub(crate) fn snapshot_outputs(update: ComposeUpdate) -> (ComposeUpdate, DisplayList) {
    let display_list = match &update {
        ComposeUpdate::Full { display_list, .. } => display_list.clone(),
        ComposeUpdate::Delta { display_list, .. } => display_list.clone(),
    };
    (update, display_list)
}

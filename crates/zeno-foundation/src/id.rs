use std::sync::atomic::{AtomicU64, Ordering};

use zeno_ui::NodeId;

static NEXT_NODE_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_node_id() -> NodeId {
    NodeId(NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed))
}

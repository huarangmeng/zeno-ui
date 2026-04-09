//! relayout 入口保留在 render 层，但具体 measured 兼容逻辑已经下沉到 layout 模块。

use super::*;

pub(super) fn relayout_layout(
    node: &Node,
    origin: Point,
    available: Size,
    text_system: &dyn TextSystem,
    retained: &RetainedComposeTree,
    layout_dirty_roots: &[NodeId],
) -> crate::layout::LayoutArena {
    crate::layout::relayout_layout(
        node,
        origin,
        available,
        text_system,
        retained,
        layout_dirty_roots,
    )
}

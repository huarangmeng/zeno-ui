//! retained tree 负责缓存 Compose 的布局、片段与脏区状态。

mod dirty_roots;
mod indexing;
mod retained;
#[cfg(test)]
mod tests;

pub use retained::RetainedComposeTree;

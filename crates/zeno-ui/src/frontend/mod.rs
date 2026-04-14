//! Frontend 层：把声明式 Node 树编译为扁平对象表与关系表。
//!
//! 目标：
//! - 将 UI identity（NodeId/Key）与 runtime identity（对象索引）隔离
//! - 为 layout/draw/patch 提供 cache-friendly 的稠密数据面
//! - 为后续工作队列式布局与批处理打基础

mod dirty_table;
mod object_table;

pub(crate) use dirty_table::{DirtyBits, DirtyTable};
pub use object_table::ElementId;
pub(crate) use object_table::{
    FrontendObject, FrontendObjectKind, FrontendObjectTable, compile_object_table,
};

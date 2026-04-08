# Logging

## 目标

- 收拢所有运行期输出，禁止业务路径继续直接使用 `println!` 或 `eprintln!`。
- 使用统一门面承接 `shell / runtime / demo` 的诊断事件，便于后续扩展到更多 crate。
- 默认输出保持简洁；在 debug 场景允许完整打印字段与源码位置信息。

## 终态

- 日志系统采用四层入口：基础级别宏、语义普通日志宏、错误对象宏、语义错误对象宏。
- SDK 只暴露稳定语义入口，不暴露初始化配置，也不鼓励直接使用内部原始错误事件宏。
- 普通运行日志按 `frame / session / runtime` 收敛 target。
- 错误日志按 `backend / session / window / runtime` 收敛 target。
- 错误日志字段统一从错误对象读取，`error_code` 由统一错误码表提供。
- 错误码表与日志宏分离维护：调用方只选对语义宏和错误对象，不在调用点拼语义。

## 统一入口

- SDK 不对外暴露日志配置类型和初始化函数，首次打印时自动完成内部初始化。
- 公共打印入口位于 `zeno_core` 导出的日志宏，分为基础级别宏与语义分类宏两层。
- 日志级别优先级仍为 `ZENO_LOG` -> `RUST_LOG` -> 代码默认值。
- 输出策略直接感知当前构建模式：
  - release 使用紧凑单行输出，默认控制日志量。
  - debug 使用完整诊断输出，自动附带源码位置、线程信息和更多字段。

## 分类入口

- 基础级别宏：`zeno_trace! / zeno_debug! / zeno_info! / zeno_warn! / zeno_error!`
- 语义分类宏：`zeno_frame_log!(level, ...) / zeno_session_log!(level, ...) / zeno_runtime_log!(level, ...)`
- 错误对象宏：`zeno_warn_error!(event, error, ...) / zeno_error_error!(event, error, ...)`
- 语义错误对象宏：`zeno_backend_warn! / zeno_backend_error! / zeno_session_warn! / zeno_session_error! / zeno_window_warn! / zeno_window_error! / zeno_runtime_warn! / zeno_runtime_error!`
- 原始错误事件宏保留为内部基础能力，不作为 SDK 常规入口暴露。
- 业务代码优先使用语义分类宏；涉及错误对象时统一使用错误对象宏。

## 级别约定

- `error`：初始化失败、后端不可用、运行无法恢复的错误。
- `warn`：发生降级、回退或非预期但可继续运行的状态。
- `info`：生命周期摘要、后端选择结果、一次会话级统计。
- `debug`：默认开发期诊断信息，例如逐帧统计。
- `trace`：高频或更细粒度的内部事件，按需通过环境变量放开。

## 字段约定

- 帧日志统一包含 `frame`、`backend`、`command_count`、`resource_count`。
- 调度相关状态统一包含 `layout`、`paint`、`present`。
- 资源缓存摘要统一走 `cache` 字段。
- 错误日志统一包含 `event`、`error_code`、`component`、`op`、`status`、`error_kind`、`message`。
- 会话类日志优先带 `backend`、`window_id`、`attempts`、`fallback_used` 等稳定字段。
- 需要完整上下文时，在 debug profile 下附带结构体的 `Debug` 输出。

## 错误模型

- `ZenoError` 不再使用裸字符串配置错误，而是统一携带 `error_code / component / operation / message`。
- `BackendUnavailableReason::RuntimeProbeFailed` 也已结构化，统一携带稳定错误码与操作阶段。
- 日志记录错误时优先使用错误对象宏，让 `error_code / component / operation / error_kind / message` 自动从错误对象读取。
- backend / session / window 三层共享同一份 `ZenoErrorCode` 表，避免同类问题在不同模块里出现不同字符串语义。
- backend 解析错误继续使用专用变体，窗口、会话、后端初始化错误统一映射到结构化 `invalid_configuration`。
- 错误码明细与分层语义见 [error-codes.md](file:///Users/bytedance/RustroverProjects/zeno-ui/docs/architecture/error-codes.md)。

## 使用约束

- 有 `ZenoError` 或其他统一错误对象时，禁止手写 `error_code / component / op / error_kind / message`，必须走错误对象宏。
- 仅在“没有错误对象、但需要上报错误级事件”的场景下，才允许使用内部原始错误事件宏。

## 使用方式

```rust
zeno_ui::zeno_runtime_log!(debug, app = "demo", "runtime ready");
zeno_ui::zeno_session_log!(info, backend = "skia", "session ready");
zeno_ui::zeno_backend_error!(
    "backend_resolution_failed",
    error,
    status = "fail",
    "backend resolution failed"
);
```

```bash
ZENO_LOG=trace cargo run -p minimal_app
```

## 演进方向

- 后续新增 crate 时，只允许接入项目日志宏，不直接依赖具体日志库宏。
- 若未来需要文件落盘、JSON 输出或遥测上报，只需要替换内部日志适配层。

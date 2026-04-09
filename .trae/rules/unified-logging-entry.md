# Unified Logging Entry

## 运行期日志必须统一走 zeno-core 门面
- 业务代码禁止直接使用 `println!`、`eprintln!` 输出运行期日志，也不应绕过门面直接依赖底层日志宏。
- 新增日志时，统一使用 `zeno-core` 导出的项目日志宏；普通事件优先使用语义分类宏，错误事件优先使用错误对象宏。
- 如果已经有统一错误对象，禁止在调用点手写 `error_code`、`component`、`op`、`error_kind`、`message`，必须通过错误对象宏自动展开字段。
- 新增 crate 或模块时，只允许接入项目统一日志入口，避免日志语义、target 和字段约定再次分叉。

# Error Codes

## 目标

- 为 `backend / session / window / ui_runtime / mobile` 提供稳定、可搜索、可观测的统一错误码。
- 让错误对象、日志字段和文档语义保持一一对应，不再依赖字符串约定。
- 为后续告警、埋点、遥测和问题分派提供固定主键。

## 日志系统终态

- 初始化由内部自动完成，SDK 不暴露日志初始化配置。
- 基础日志入口只负责级别与结构化字段。
- 语义日志入口分为普通事件宏与错误对象宏两层。
- 普通运行日志按域区分为 `frame / session / runtime`。
- 错误日志按域区分为 `backend / session / window / runtime`。
- 原始错误事件宏只作为内部基础能力保留，不作为常规业务入口。
- 有错误对象时，统一走语义错误宏，不允许手写 `error_code / component / op / error_kind / message`。

## 错误码表

| 错误码 | 层级 | 含义 |
| --- | --- | --- |
| `backend.unavailable` | backend | 某个后端不可用 |
| `backend.none_available` | backend | 所有候选后端均不可用 |
| `backend.not_implemented_for_platform` | backend | 当前平台未实现该后端 |
| `backend.missing_platform_surface` | backend | 平台 surface 缺失 |
| `backend.missing_gpu_context` | backend | GPU context 缺失 |
| `backend.explicitly_disabled` | backend | 后端被显式关闭 |
| `backend.probe_unknown_platform` | backend | probe 阶段遇到未知平台 |
| `backend.probe_unavailable_without_reason` | backend | probe 返回不可用但没有原因 |
| `backend.renderer_create_failed` | backend | renderer 创建失败 |
| `backend.skia_surface_create_failed` | backend | Skia surface 创建失败 |
| `backend.impeller_shader_compile_failed` | backend | Impeller shader 编译失败 |
| `backend.impeller_render_pass_attachment_missing` | backend | Impeller render pass 缺少 attachment |
| `backend.impeller_color_pipeline_function_missing` | backend | Impeller color pipeline shader 获取失败 |
| `backend.impeller_color_pipeline_attachment_missing` | backend | Impeller color pipeline attachment 缺失 |
| `backend.impeller_color_pipeline_state_create_failed` | backend | Impeller color pipeline state 创建失败 |
| `backend.impeller_text_pipeline_function_missing` | backend | Impeller text pipeline shader 获取失败 |
| `backend.impeller_text_pipeline_attachment_missing` | backend | Impeller text pipeline attachment 缺失 |
| `backend.impeller_text_pipeline_state_create_failed` | backend | Impeller text pipeline state 创建失败 |
| `session.create_render_session_failed` | session | 渲染会话创建失败 |
| `session.invalid_window_width` | session | 窗口宽度非法 |
| `session.invalid_window_height` | session | 窗口高度非法 |
| `session.wrap_render_target_failed` | session | render target 包装失败 |
| `session.swap_buffers_failed` | session | swap buffers 失败 |
| `session.next_drawable_unavailable` | session | drawable 获取失败 |
| `window.create_event_loop_failed` | window | 事件循环创建失败 |
| `window.run_app_failed` | window | 窗口事件循环运行失败 |
| `window.feature_disabled` | window | 必需 window feature 未开启 |
| `window.renderer_unavailable` | window | 窗口绘制阶段 renderer 不可用 |
| `ui_runtime.root_not_set` | ui_runtime | UI 根节点未设置 |
| `ui_runtime.viewport_not_configured` | ui_runtime | viewport 未配置 |
| `mobile.viewport_invalid` | mobile | 移动端 viewport 非法 |

## 使用规范

- backend 相关错误优先使用 `zeno_backend_warn! / zeno_backend_error!`
- session 相关错误优先使用 `zeno_session_warn! / zeno_session_error!`
- window 相关错误优先使用 `zeno_window_warn! / zeno_window_error!`
- runtime 相关错误优先使用 `zeno_runtime_warn! / zeno_runtime_error!`
- 只有没有错误对象时，才允许回落到内部原始错误事件宏

## 演进原则

- 新增错误时，先补 `ZenoErrorCode`，再落错误对象和日志调用点。
- 同类错误跨模块复用已有错误码，不重复发明接近但不同的名字。
- 若错误需要拆子类，优先扩展 `operation` 或附加字段，不轻易改动已有错误码字符串。

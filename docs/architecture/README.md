# Architecture Docs

## 文档状态总览

| 文档 | 主题 | 当前状态 | 建议动作 |
| --- | --- | --- | --- |
| `rendering-architecture.md` | 总体分层与渲染主链路 | 已完成主链路梳理 | 保留，继续同步增量渲染与移动端 session 进展 |
| `backend-selection.md` | 后端选择策略与平台矩阵 | 进行中 | 保留并更新平台状态 |
| `platform-shell.md` | Shell 职责与宿主边界 | 进行中 | 保留并更新当前实现 |
| `desktop-rendering.md` | 桌面渲染链路与双后端 presenter | 进行中 | 保留，并把下一步聚焦到 Impeller 非 macOS 与更细粒度局部提交 |
| `compose-layer.md` | 声明式组件层与 Scene 生成 | 已完成 MVP 主链路 | 保留，继续记录 retained / patch / dirty roots 的下一阶段优化 |
| `logging.md` | 统一日志系统与级别约定 | 已完成首版方案 | 保留，作为后续扩展 subscriber 的基线 |
| `error-codes.md` | 统一错误码与语义错误日志规范 | 已完成首版方案 | 保留，作为日志/遥测/告警的稳定主键表 |
| `performance-plan.md` | 性能与开发体验优化路线 | 进行中 | 保留，作为后续架构收敛的执行参考 |
| `roadmap.md` | 阶段路线图 | 进行中 | 保留并按当前完成度持续收敛 |

## 当前共识

- 代码已经从“纯骨架期”进入“统一 session + retained/patch MVP”阶段。
- `zeno-ui -> RenderSceneUpdate -> runtime -> shell -> backend` 主链路已经打通。
- 桌面 Skia 路径稳定可用，macOS 上 Impeller 已具备 Metal presenter。
- 移动端 shell 已具备 `session binding / attachment / presenter interface / render session` 主链路，Android/iOS 的 presenter 创建接口已经固定。
- 下一阶段的重点不再是补主链路，而是继续细化 dirty root、局部 GPU 提交、文本主路径与工程化验证工具。

## 已完成

- Workspace 已按 `core / graphics / runtime / shell / compose / text / backend-*` 垂直拆分。
- Runtime 已实现 Impeller 优先、Skia 兜底的后端选择策略。
- `zeno-backend-skia` 已提供真实 Scene 到 Skia Canvas 的翻译路径。
- `zeno-platform` 已收敛出统一平台集成层，支持桌面 presenter 路径与移动端 `binding / attachment / presenter builder / render session` 链路。
- `zeno-ui` 已具备 retained tree、dirty propagation、layout dirty roots、paint-only patch 与 `RenderSceneUpdate` 提交模型。
- 根 crate 已提供 `macos`、`linux`、`windows`、`android`、`ios` 平台 preset feature。

## 进行中

- `zeno-text` 已补上 system shaping（rustybuzz）、paragraph cache 与后端共享 glyph 栅格缓存；下一步转向更完整的 shaping 覆盖与更强的缓存/统计体系。
- `zeno-scene::Scene` 已具备 layer/clip/transform/effect/offscreen 的 retained compositor MVP；下一步转向更复杂 filter graph、多级 effect fusion 与资源句柄化。
- Impeller 的真实能力目前仍主要集中在 macOS；移动端虽已有 presenter 适配层，但真实 swapchain / drawable / command buffer 生命周期仍未完全落地，非 macOS 桌面 Impeller 也仍未完成。
- 已补 `examples/text_probe` 与 `examples/bench_gallery` 并提供 bench suite 脚本与 CI workflow；后续重点转向 golden image、更多场景覆盖与基线管理策略。

## 下一步整理原则

- 统一文档语言与阶段描述，优先使用“已完成 / 进行中 / 下一步”三个状态。
- 文档应以当前代码为准，不再保留已经失效的“占位实现”表述。
- 桌面后端相关内容集中到 `desktop-rendering.md`。
- 路线图只记录真正还未完成的工作，不重复已经完成的架构拆分。
- `performance-plan.md` 负责记录“下一阶段先做什么、为什么做、按什么顺序做”。
- V2 对象表架构已在当前代码线上原地落地（`FrontendObjectTable` / `DirtyTable` / `LayoutWorkQueue` / 对象 diff reconcile / index-first scene & patch），相关设计已融入 `rendering-architecture.md`、`compose-layer.md` 与 `performance-plan.md`。

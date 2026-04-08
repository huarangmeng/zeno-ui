# Architecture Docs

## 文档状态总览

| 文档 | 主题 | 当前状态 | 建议动作 |
| --- | --- | --- | --- |
| `rendering-architecture.md` | 总体分层与渲染主链路 | 已完成基础分层梳理 | 保留，补充当前阶段判断 |
| `backend-selection.md` | 后端选择策略与平台矩阵 | 进行中 | 保留并更新平台状态 |
| `platform-shell.md` | Shell 职责与宿主边界 | 进行中 | 保留并更新当前实现 |
| `desktop-rendering.md` | 桌面渲染链路与双后端 presenter | 进行中 | 保留并吸收 Impeller 桌面状态 |
| `compose-layer.md` | 声明式组件层与 Scene 生成 | 进行中 | 保留并更新下一阶段重点 |
| `logging.md` | 统一日志系统与级别约定 | 已完成首版方案 | 保留，作为后续扩展 subscriber 的基线 |
| `error-codes.md` | 统一错误码与语义错误日志规范 | 已完成首版方案 | 保留，作为日志/遥测/告警的稳定主键表 |
| `performance-plan.md` | 性能与开发体验优化路线 | 进行中 | 保留，作为后续架构收敛的执行参考 |
| `roadmap.md` | 阶段路线图 | 进行中 | 保留并按当前完成度重写 |

## 当前共识

- 代码已经从“纯骨架期”进入“桌面双后端原型期”。
- `zeno-compose -> Scene -> runtime -> shell -> backend` 主链路已经打通。
- Skia 已具备真实桌面绘制路径，macOS 上 Impeller 已具备 Metal presenter 原型。
- 下一阶段的重点不再是继续增加命令种类，而是补齐 retained tree、脏区驱动、资源缓存和统一的渲染会话抽象。

## 已完成

- Workspace 已按 `core / graphics / runtime / shell / compose / text / backend-*` 垂直拆分。
- Runtime 已实现 Impeller 优先、Skia 兜底的后端选择策略。
- `zeno-backend-skia` 已提供真实 Scene 到 Skia Canvas 的翻译路径。
- `zeno-shell` 已收敛出桌面 presenter 分发点，支持 Skia 与 macOS Impeller 两条桌面路径。

## 进行中

- `zeno-compose` 仍是全树测量与全量 Scene 发射，尚未进入 retained tree 模式。
- `zeno-text` 仍以 fallback 测量为主，缺少真实 shaping 与缓存抽象。
- `zeno-graphics::Scene` 仍是扁平命令流，尚未支持资源句柄、layer、clip、局部更新。
- Shell 与 runtime 之间仍存在“解析 backend”和“再次按 backend 分发 presenter”的双重抽象。

## 下一步整理原则

- 统一文档语言与阶段描述，优先使用“已完成 / 进行中 / 下一步”三个状态。
- 文档应以当前代码为准，不再保留已经失效的“占位实现”表述。
- 桌面后端相关内容集中到 `desktop-rendering.md`。
- 路线图只记录真正还未完成的工作，不重复已经完成的架构拆分。
- `performance-plan.md` 负责记录“下一阶段先做什么、为什么做、按什么顺序做”。

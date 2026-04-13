# Desktop Rendering

## 状态
- 状态：进行中
- 阶段判断：桌面双后端 presenter 已经成型，Skia 与 macOS Impeller 都已具备原生 `DisplayList` 提交路径；整体已从 retained-only presenter 阶段推进到 `DisplayList` 主协议阶段，但仍属于桌面验证阶段。

## 当前桌面链路
- `zeno-runtime` 负责按平台与配置解析目标 backend。
- `ResolvedSession` 作为统一的桌面 session descriptor，显式携带 `platform / backend / attempts / frame_stats`。
- `zeno-platform::DesktopShell::run_pending_scene_window` 负责事件循环、窗口创建、surface 生命周期与 presenter 启动。
- `DesktopSessionPlan` 统一桌面后端分发点，目前包含 Skia GL session 和 macOS Impeller Metal session。
- `zeno-ui` / `UiRuntime` 输出对 retained scene 的借用型更新与 `DisplayList` 快照，最终由 `AppHost` 按 `RenderCapabilities::display_list_submit` 选择具体 presenter 的提交路径。

## 已完成

### Skia
- `zeno-backend-skia` 已提供原生 `DisplayList` -> Skia Canvas 翻译路径，当前支持基础图元、文本、图像、clip chain、stacking context、blur 与 drop-shadow。
- 桌面 Skia 路径通过 GL-backed Skia surface 呈现，`minimal_app` 可以直接验证。
- Skia session 已支持 patch 路径的局部提交（局部清屏 + 区域绘制），并可直接消费 `DisplayList`。

### Impeller
- macOS 已具备 `ImpellerMetalSession`，可创建 `MetalLayer` 并提交 drawable。
- `zeno-backend-impeller` 已具备原生 `DisplayList` 的 `MetalSceneRenderer`，支持基础图元、文本、图像、多级 clip chain 交集裁剪、stacking context 递归与 offscreen 合成。
- patch 路径支持 `MTLLoadAction::Load` + 根级 scissor，将脏区下沉到 GPU；局部提交不再通过 `snapshot_scene()` / `partial_scene_for_dirty_bounds()` 或 `DisplayList -> Scene` 桥接回退。

## 当前问题
- Skia 的桌面 GPU 呈现主要在 shell 中完成，backend 自身更偏 retained 翻译层，而不是完整 session。
- Impeller 的桌面绘制能力现已直接集中在 macOS presenter 与原生 `DisplayList` `MetalSceneRenderer`，不再维护旧 Scene 主路径，也不再通过桥接 scene 中转。
- 非 macOS 平台的 Impeller presenter 仍未实现；当前桌面 session plan 也已与 runtime probe 对齐，避免 Linux/Windows 上再出现“描述符可建但 presenter 不可用”的假阳性路径。
- 当前剩余重点已转向更完整的 compositor 基础设施：`DamageTracker`、tile cache、独立 compositor 层、更复杂 effect graph 与非 macOS Impeller presenter。

## 文档合并说明
- 原 `impeller-desktop.md` 的内容已并入本文件，不再单独维护桌面 Impeller 状态文档。
- 后续所有桌面后端进展统一记录在本文件，避免 Skia / Impeller 桌面状态重复或冲突。

## 下一步
- 继续补齐非 macOS 平台的 Impeller presenter 与真实渲染路径。
- 继续增强 Impeller 的局部 GPU 提交能力，而不是只保留全量为主的提交模型。
- 为 Skia 与 Impeller 都补齐更稳定的资源缓存、文本缓存和帧内统计，避免 presenter 层只承担“能画出来”的职责。

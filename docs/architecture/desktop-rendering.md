# Desktop Rendering

## 状态
- 状态：进行中
- 阶段判断：桌面双后端 presenter 已经成型，Skia 路径可用，macOS 的 Impeller Metal presenter 已有真实绘制原型，但整体仍属于桌面验证阶段。

## 当前桌面链路
- `zeno-runtime` 负责按平台与配置解析目标 backend。
- `ResolvedSession` 作为统一的桌面 session descriptor，显式携带 `platform / backend / attempts / frame_stats`。
- `zeno-shell::DesktopShell::run_pending_scene_window` 负责事件循环、窗口创建、surface 生命周期与 presenter 启动。
- `DesktopSessionPlan` 统一桌面后端分发点，目前包含 Skia GL session 和 macOS Impeller Metal session。
- `zeno-compose` 输出的 `Scene` 最终由具体 presenter 交给对应 backend 路径执行。

## 已完成

### Skia
- `zeno-backend-skia` 已提供真实 `Scene -> Skia Canvas` 翻译路径，支持 `Clear / Fill / Stroke / Text`。
- 桌面 Skia 路径通过 GL-backed Skia surface 呈现，`minimal_app` 可以直接验证。

### Impeller
- macOS 已具备 `ImpellerMetalSession`，可创建 `MetalLayer` 并提交 drawable。
- `zeno-backend-impeller` 已具备 `MetalSceneRenderer`，可以处理基础形状和文本纹理绘制。

## 当前问题
- Skia 的桌面 GPU 呈现主要在 shell 中完成，backend 自身更偏 Scene 翻译层，而不是完整 session。
- Impeller 的 `Renderer` trait 实现仍偏占位，真正的桌面绘制能力集中在 macOS presenter 与 `MetalSceneRenderer`。
- 非 macOS 平台的 Impeller presenter 仍未实现，因此桌面 Impeller 真实能力依旧只有 macOS 成熟。

## 文档合并说明
- 原 `impeller-desktop.md` 的内容已并入本文件，不再单独维护桌面 Impeller 状态文档。
- 后续所有桌面后端进展统一记录在本文件，避免 Skia / Impeller 桌面状态重复或冲突。

## 下一步
- 将同一套 session factory 继续外推到移动端，而不是只覆盖桌面路径。
- 将桌面帧循环从持续重绘改为事件驱动 + 动画驱动 + dirty 驱动。
- 为 Skia 与 Impeller 都补齐资源缓存、文本缓存和帧内统计，避免 presenter 层只承担“能画出来”的职责。

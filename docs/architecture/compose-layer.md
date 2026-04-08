# Compose Layer

## Goal
- 在渲染抽象之上增加一层声明式 UI 节点树。
- 让上层 API 更接近 Compose/SwiftUI 风格，而不是直接手写 `DrawCommand`。
- 保持组件层与宿主层解耦，库默认不依赖桌面开窗能力。

## Current Scope
- `Node` 作为统一声明式节点。
- `text`、`container`、`column`、`row`、`spacer` 作为首批基础构件。
- `Style` 提供 padding、background、foreground、corner radius、spacing、fixed size。
- `ComposeRenderer` 负责把节点树测量并转换为 `Scene`。

## Flow
1. 上层通过声明式节点 API 构建组件树。
2. 组件树进入最小布局系统，基于 viewport 和文本测量得到 frame。
3. `ComposeRenderer` 把测量结果翻译为 `DrawCommand`。
4. 现有 runtime 按平台自动选择 Impeller 或 Skia 后端去渲染 scene。

## Next Steps
- 增加更多基础组件，例如 `button`、`surface`、`image`。
- 引入 modifier 链式模型，而不是把常见样式都直接挂在 `Node` 上。
- 增加状态驱动与重组模型，逐步靠近真正 Compose 风格 runtime。

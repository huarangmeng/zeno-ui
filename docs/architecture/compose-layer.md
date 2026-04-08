# Compose Layer

## 状态
- 状态：进行中
- 阶段判断：声明式节点树、最小布局与 `Scene` 生成已经成立，但当前仍是“全树测量 + 全量命令发射”的即时模型，还不是 Compose 风格的 retained runtime。

## Goal
- 在渲染抽象之上增加一层声明式 UI 节点树。
- 让上层 API 更接近 Compose/SwiftUI 风格，而不是直接手写 `DrawCommand`。
- 保持组件层与宿主层解耦，库默认不依赖桌面开窗能力。

## Current Scope
- `Node` 作为统一声明式节点。
- `text`、`container`、`column`、`row`、`spacer` 作为首批基础构件。
- `Style` 提供 padding、background、foreground、corner radius、spacing、fixed size。
- `ComposeRenderer` 负责把节点树测量并转换为 `Scene`。
- 当前文本测量仍依赖 `TextSystem`，默认实现是 fallback 估算模型。

## Flow
1. 上层通过声明式节点 API 构建组件树。
2. 组件树进入最小布局系统，基于 viewport 和文本测量得到 frame。
3. `ComposeRenderer` 把测量结果翻译为 `DrawCommand`。
4. runtime 解析 backend，桌面 shell 再将 scene 交给具体 presenter 渲染。

## 当前限制
- 任意变化都会触发整棵树重新测量。
- `Scene` 每次都是从头生成，没有 retained tree、脏区传播和局部更新能力。
- 文本布局结果在 `Scene` 发射阶段仍会复制，缺少缓存与共享引用结构。

## Next Steps
- 引入稳定 `NodeId`、dirty reason 和 retained tree。
- 把布局与绘制从全量重建演进为 dirty subtree 更新。
- 引入 modifier 链式模型，而不是把常见样式都直接挂在 `Node` 上。
- 增加状态驱动、重组模型和更丰富的基础组件。

# Compose Layer

## 状态
- 状态：进行中
- 阶段判断：声明式节点树、最小布局、retained tree 与 `SceneSubmit` 生成已经成立，当前已进入 retained runtime 的 MVP 阶段，但仍需继续细化 dirty root、局部提交与更高阶 scene 结构。

## Goal
- 在渲染抽象之上增加一层声明式 UI 节点树。
- 让上层 API 更接近 Compose/SwiftUI 风格，而不是直接手写 `DrawCommand`。
- 保持组件层与宿主层解耦，库默认不依赖桌面开窗能力。

## Current Scope
- `Node` 作为统一声明式节点。
- `text`、`container`、`column`、`row`、`spacer` 作为首批基础构件。
- `Style` 提供 padding、background、foreground、corner radius、spacing、fixed size。
- `ComposeRenderer` / `ComposeEngine` 负责把节点树测量并转换为 `SceneSubmit`。
- 当前文本测量仍依赖 `TextSystem`，默认实现是 fallback 估算模型。

## Flow
1. 上层通过声明式节点 API 构建组件树。
2. retained tree 基于 `NodeId`、dirty flags 与 cached layout 决定是否走局部 relayout 或 paint-only 快路径。
3. 组件树进入布局系统，基于 viewport 和文本测量得到 frame。
4. `ComposeRenderer` 把测量结果翻译为 `SceneSubmit`，必要时生成 `ScenePatch`。
5. keyed rebuild 会先按 `NodeId` 做局部 reconcile，再决定走 cache hit、局部 relayout 或 patch 提交。
6. runtime 解析 backend，shell 再将结构化 scene 提交给具体 render session。

## 当前限制
- keyed rebuild 目前依赖稳定 `NodeId`；未显式 `.key()` 的节点仍会退化为更粗粒度更新。
- dirty root 归并目前仍是 MVP，兄弟影响范围与更细粒度祖先裁剪仍可继续细化。
- `Scene` 已支持 block / patch，但还没有继续演进到 layer、clip、transform 等更高阶结构。
- 文本布局结果在 `Scene` 发射阶段仍会复制，缺少缓存与共享引用结构。

## Next Steps
- 继续细化 dirty root 合并策略与局部 relayout 影响范围。
- 继续细化 keyed reconcile 的 dirty reason 判断，让更多 rebuild 降级为 paint-only 或更小 patch。
- 引入 modifier 链式模型，而不是把常见样式都直接挂在 `Node` 上。
- 增加状态驱动、重组模型和更丰富的基础组件。

# Compose Layer

## 状态
- 状态：进行中
- 阶段判断：声明式节点树、最小布局、retained tree 与 `RetainedDisplayList` 生成已经成立，当前正式运行时协议已经完成向 `DisplayList` 单轨提交的切换。已完成首批局部化与一致性修复：paint-only 与局部 relayout 会按对象索引更新 `RetainedDisplayList`；macOS Shell 支持基于 dirty-bounds 的局部提交；keyed child reorder 会尽量收敛为局部 display-list 更新；layout/text dirty root 会尽量保留在叶子与局部分支。

## Goal
- 在渲染抽象之上增加一层声明式 UI 节点树。
- 让上层 API 更接近 Compose/SwiftUI 风格，而不是直接手写 `DrawCommand`。
- 保持组件层与宿主层解耦，库默认不依赖桌面开窗能力。

## Current Scope
- `Node` 作为统一声明式节点。
- `zeno-ui` 保留节点模型、modifier、layout、retained tree 与 display-list 翻译；`text / container / box / column / row / spacer` 等首批基础构件已迁入 `zeno-foundation`，作为更接近 Compose Foundation 的稳定入口。
- `Modifier` 链是节点装饰的唯一真相源；padding、background、foreground、font size、corner radius、spacing、fixed size、content alignment、stack arrangement、stack cross-axis alignment、clip、2D transform、transform origin、opacity、layer、effect 都在布局/绘制阶段按需解析。
- `ComposeRenderer` / `ComposeEngine` 负责把节点树测量并转换为 `RetainedDisplayList + DisplayList` 更新；测试侧也已经统一改为 `DisplayList` 断言。
- 当前文本测量依赖 `TextSystem`；默认门面路径可选择 fallback/system shaping 实现。

## Flow
1. 上层通过声明式节点 API 构建组件树。
2. retained tree 在 frontend compile 阶段将 `Node` 树编译为 `FrontendObjectTable`（稠密对象表），再由 `DirtyTable`（bitset + generation）管理 style/intrinsic/layout/paint/display_list/resource 六种脏类型，决定是否走局部 relayout 或 paint-only 快路径。`NodeId` 仅在此阶段提供 keyed identity。
3. `LayoutWorkQueue` 以两阶段工作队列（intrinsic + placement）驱动布局，按对象类型批处理（Text/Spacer/Container/Box/Stack），不再从 `Node` 树递归。
4. `ComposeRenderer` 基于 `FrontendObjectTable` 显式栈遍历构建 `RetainedDisplayList`，由 `SpatialTree / ClipChainStore / StackingContext / DisplayItem` 承载 transform / clip / opacity / blend / effect / offscreen 语义。
5. keyed rebuild 会先做 keyed identity 对照；reconcile 基于新旧 `FrontendObjectTable` 对象 diff，而非 Node 树递归；display-list 更新与 repaint 全部 index-first，`NodeId` 仅收敛在 keyed identity 边界。
6. `UiRuntime` 与 runtime/shell 闭环会决定何时重组、何时提交；外部 app 只声明 `AppView`，不直接感知任何旧 scene/delta 提交协议。

## 当前限制
- keyed rebuild 目前依赖稳定 `NodeId`；未显式 `.key()` 的节点仍会退化为更粗粒度更新。
- modifier 已覆盖样式、clip、完整 2D transform、transform origin、opacity、显式 layer 以及 blend / blur / drop shadow effect 链，但还没有扩展到 gesture、semantics 与更复杂 effect 参数。
- dirty root 归并已从单纯祖先去重推进到“最小容器根 + 同父结构/顺序脏根合并”策略：layout/text 兄弟节点仍可作为独立脏根，结构变更与顺序变更会尽量收敛到最小共同容器根，并避免无意义升级到更高祖先；当前剩余问题主要集中在更复杂 effect tree 与更高阶结构 patch 类型。
- `DisplayList` 已支持 `SpatialTree + ClipChainStore + StackingContext + DisplayItemPayload`，并具备 subtree clip / opacity / transform origin / effect stack 驱动的正式渲染基础；当前主要待补的是 filter graph、effect fusion 与更复杂 effect tree。
- 文本布局结果在 `DisplayList` 发射阶段仍会复制，缺少缓存与共享引用结构。
- 跨 crate 协议层已切换为 `LayerObject/RenderObject`，`node_id` 仅在调试/对照表边界可见；runtime identity 已全面切换为对象表稠密索引。
- 文本对象尚未独立维护 `TextObjectTable`（paragraph hash / shaping handle / glyph run handle），当前文本测量结果在 layout / display-list / backend 之间仍存在复制；后续可引入独立文本对象表以实现 paragraph cache、glyph cache 的跨阶段共享。

## Next Steps
- 继续把当前“最小容器根 + 结构 patch”策略扩展到更复杂的 layer/effect tree，进一步减少高阶结构编辑时的 patch 面积。
- 在现有 keyed reconcile 基础上继续扩展结构化 patch 类型，把更多 layer/effect 级编辑压缩为更小增量，而不是回退到更粗粒度 rebuild。
- 把 modifier 从当前样式/compositor/effect 链继续扩展为可承载 filter graph、gesture 与交互语义的通用节点装饰模型。
- 在 `zeno-foundation` 中继续扩展 scroll、basic controls 与交互基础组件，并把更高层 design system 留给后续独立层。

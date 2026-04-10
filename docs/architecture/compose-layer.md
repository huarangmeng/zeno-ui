# Compose Layer

## 状态
- 状态：进行中
- 阶段判断：声明式节点树、最小布局、retained tree 与 `SceneSubmit` 生成已经成立，当前已进入 retained runtime 的 MVP 阶段。已完成首批局部化与一致性修复：patch 路径与 full 路径的 layer/offscreen 判定已统一；paint-only、结构不变的 relayout，以及 keyed child insert/remove 的结构变更都将按脏子树生成 `ScenePatch`；macOS Shell 在多层场景下也支持基于 dirty-bounds 的局部提交；keyed child reorder 已收敛为 order-only patch；layout/text dirty root 会尽量保留在叶子与局部分支；Stack reorder 场景已优先复用既有测量结果并仅在必要时扩大 patch。

## Goal
- 在渲染抽象之上增加一层声明式 UI 节点树。
- 让上层 API 更接近 Compose/SwiftUI 风格，而不是直接手写 `DrawCommand`。
- 保持组件层与宿主层解耦，库默认不依赖桌面开窗能力。

## Current Scope
- `Node` 作为统一声明式节点。
- `zeno-ui` 保留节点模型、modifier、layout、retained tree 与 scene 翻译；`text / container / box / column / row / spacer` 等首批基础构件已迁入 `zeno-foundation`，作为更接近 Compose Foundation 的稳定入口。
- `Modifier` 链是节点装饰的唯一真相源；padding、background、foreground、font size、corner radius、spacing、fixed size、content alignment、stack arrangement、stack cross-axis alignment、clip、2D transform、transform origin、opacity、layer、effect 都在布局/绘制阶段按需解析。
- `ComposeRenderer` / `ComposeEngine` 负责把节点树测量并转换为 `SceneSubmit`。
- 当前文本测量依赖 `TextSystem`；默认门面路径可选择 fallback/system shaping 实现。

## Flow
1. 上层通过声明式节点 API 构建组件树。
2. retained tree 基于稳定 `NodeId` 做 identity，对运行时热路径则通过统一 `NodeIndexTable`、index-based dirty flags、layout dirty roots 与 cached layout 决定是否走局部 relayout 或 paint-only 快路径。
3. 组件树进入布局系统，基于 viewport 和文本测量得到 frame。
4. `ComposeRenderer` 把测量结果翻译为带 `SceneLayer + SceneBlock` 的结构化 `SceneSubmit`，其中 layer 承载 subtree clip / transform / opacity / blend / effect / offscreen 语义，必要时生成 `ScenePatch`。
5. keyed rebuild 会先做 keyed identity 对照，再决定走 cache hit、局部 relayout 或 patch 提交；当前 reconcile 的递归命中、fragment 更新、scene patch 与 repaint 已经切到 index-first，`NodeId` 主要收敛在 keyed identity 与跨索引表 remap 边界。
6. `UiRuntime` 与 runtime/shell 闭环会决定何时重组、何时提交；外部 app 只声明 `AppView`，不直接感知 `SceneSubmit`。

## 当前限制
- keyed rebuild 目前依赖稳定 `NodeId`；未显式 `.key()` 的节点仍会退化为更粗粒度更新。
- modifier 已覆盖样式、clip、完整 2D transform、transform origin、opacity、显式 layer 以及 blend / blur / drop shadow effect 链，但还没有扩展到 gesture、semantics 与更复杂 effect 参数。
- dirty root 归并已从单纯祖先去重推进到“最小容器根 + 同父结构/顺序脏根合并”策略：layout/text 兄弟节点仍可作为独立脏根，结构变更与顺序变更会尽量收敛到最小共同容器根，并避免无意义升级到更高祖先；当前剩余问题主要集中在更复杂 effect tree 与更高阶结构 patch 类型。
- `Scene` 已支持 `SceneLayer + SceneBlock + ScenePatch`，并具备 subtree clip / opacity / transform origin / effect stack 驱动的 retained compositor 基础；当前主要待补的是 filter graph、effect fusion 与更复杂 effect tree。
- 文本布局结果在 `Scene` 发射阶段仍会复制，缺少缓存与共享引用结构。
- `SceneLayer/SceneBlock` 等跨 crate 协议层仍保留 `node_id` 字段；若要彻底去掉 `NodeId`，需要同步重构 `zeno-scene`、platform session 与 backend 侧消费协议。

## Next Steps
- 继续把当前“最小容器根 + 结构 patch”策略扩展到更复杂的 layer/effect tree，进一步减少高阶结构编辑时的 patch 面积。
- 在现有 keyed reconcile 基础上继续扩展结构化 patch 类型，把更多 layer/effect 级编辑压缩为更小增量，而不是回退到更粗粒度 rebuild。
- 把 modifier 从当前样式/compositor/effect 链继续扩展为可承载 filter graph、gesture 与交互语义的通用节点装饰模型。
- 在 `zeno-foundation` 中继续扩展 scroll、basic controls 与交互基础组件，并把更高层 design system 留给后续独立层。

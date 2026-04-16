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
- `zeno-foundation` 已补齐第一批基础交互组件：`button / toggle_button / checkbox / switch / scroll`，当前以受控组件形态提供默认组合结构、默认视觉样式与交互角色标记，状态仍由上层 `App` 持有；`button / toggle_button / checkbox / switch` 已切到类型化控件包装，label/content slot 与 `on_click / checked / selected / on_checked_change / on_toggle` 等控件专属 API 收敛在 foundation 层，而不是暴露为通用 `Node` modifier。
- `Modifier` 链是节点装饰的唯一真相源；padding、background、corner radius、spacing、显式/约束尺寸、content alignment、stack arrangement、stack cross-axis alignment、clip、2D transform、transform origin、opacity、layer、effect 以及 interaction role / action / checked / focus / text-input capability 都在布局、绘制或 runtime 事件阶段按需解析与透传。文本相关能力不再继续堆叠零散 modifier，而是通过 `TextStyle` 汇总 `color / font_size / font_family / font_weight / italic / font_feature(s) / letter_spacing / line_height / text_align` 等排版语义，再由 `Style.text` 单一路径进入 layout / display-list / backend；`Modifier::TextStyle` 采用 merge 语义（仅覆盖已设置的字段），同时保留 `FontFamily / FontWeight / Italic / LetterSpacing / LineHeight / TextAlign` 等细粒度 modifier 作为独立入口。
- `ComposeRenderer` / `ComposeEngine` 负责把节点树测量并转换为 `RetainedDisplayList + DisplayList` 更新；测试侧也已经统一改为 `DisplayList` 断言。
- 当前文本测量依赖 `TextSystem`；默认门面路径可选择 fallback/system shaping 实现。

## Flow
1. 上层通过声明式节点 API 构建组件树。
2. retained tree 在 frontend compile 阶段将 `Node` 树编译为 `FrontendObjectTable`（稠密对象表），再由 `DirtyTable`（bitset + generation）管理 style/intrinsic/layout/paint/display_list/resource 六种脏类型，决定是否走局部 relayout 或 paint-only 快路径。`NodeId` 仅在此阶段提供 keyed identity。
3. `LayoutWorkQueue` 以两阶段工作队列（intrinsic + placement）驱动布局，按对象类型批处理（Text/Spacer/Container/Box/Stack），不再从 `Node` 树递归。
4. `ComposeRenderer` 基于 `FrontendObjectTable` 显式栈遍历构建 `RetainedDisplayList`，由 `SpatialTree / ClipChainStore / StackingContext / DisplayItem` 承载 transform / clip / opacity / blend / effect / offscreen 语义。
5. keyed rebuild 会先做 keyed identity 对照；reconcile 基于新旧 `FrontendObjectTable` 对象 diff，而非 Node 树递归；display-list 更新与 repaint 全部 index-first，`NodeId` 仅收敛在 keyed identity 边界。
6. `UiRuntime` 与 runtime/shell 闭环会决定何时重组、何时提交；当前已具备基于布局命中测试的最小交互闭环：platform 收集 pointer / native touch / keyboard / text input，runtime 将其映射为 `Click / ToggleChanged / FocusChanged / KeyInput / TextInput` 等高层语义事件回传给 app。

## 当前限制
- keyed rebuild 目前依赖稳定 `NodeId`；未显式 `.key()` 的节点仍会退化为更粗粒度更新。
- modifier 已覆盖样式、clip、完整 2D transform、transform origin、显式/约束尺寸、opacity、显式 layer、interaction role / action / checked / focus / text-input capability 以及 blend / blur / drop shadow effect 链；文本样式入口当前已收敛为 `TextStyle`（并保留 `foreground / font_size` 作为兼容 sugar），runtime 已补齐 pointer click、native touch、focus、键盘激活与文本输入分发，但 hover/pressed 生命周期、手势竞争和更高阶 semantics tree 仍未完善。
- `button / toggle_button / checkbox / switch / scroll` 当前是受控组件与组合式滚动外壳：提供默认结构、滚动裁剪与偏移表达；button/toggle/checkbox/switch 已可通过 foundation 控件 API 接入 runtime 事件回传，但还没有真正的 `scroll` wheel/drag scrolling 与更完整的手势组合。
- 这批基础交互组件目前优先暴露“类型化控件入口 + content slot + 外层样式链覆盖”模型，尚未把 indicator/knob/viewport clip 等内部 token 全量提升为公开可配置参数；容器层通过 `Into<Node>` 接纳这些控件包装，以避免 `.build()` 式显式收尾。
- dirty root 归并已从单纯祖先去重推进到“最小容器根 + 同父结构/顺序脏根合并”策略：layout/text 兄弟节点仍可作为独立脏根，结构变更与顺序变更会尽量收敛到最小共同容器根，并避免无意义升级到更高祖先；当前剩余问题主要集中在更复杂 effect tree 与更高阶结构 patch 类型。
- `DisplayList` 已支持 `SpatialTree + ClipChainStore + StackingContext + DisplayItemPayload`，并具备 subtree clip / opacity / transform origin / effect stack 驱动的正式渲染基础；当前主要待补的是 filter graph、effect fusion 与更复杂 effect tree。
- 文本布局结果在 `DisplayList` 发射阶段仍会复制，缺少缓存与共享引用结构。
- 跨 crate 协议层已切换为 `LayerObject/RenderObject`，`node_id` 仅在调试/对照表边界可见；runtime identity 已全面切换为对象表稠密索引。
- 文本对象尚未独立维护 `TextObjectTable`（paragraph hash / shaping handle / glyph run handle），当前文本测量结果在 layout / display-list / backend 之间仍存在复制；后续可引入独立文本对象表以实现 paragraph cache、glyph cache 的跨阶段共享。
- `TextStyle` 当前已覆盖 `color / font_size / font_family / font_weight / italic / font_feature(s) / letter_spacing / line_height / text_align`，但 `Typography` 主题层与 OpenType 特性的真实 shaping 映射仍待补齐。

## Next Steps
- 继续把当前“最小容器根 + 结构 patch”策略扩展到更复杂的 layer/effect tree，进一步减少高阶结构编辑时的 patch 面积。
- 在现有 keyed reconcile 基础上继续扩展结构化 patch 类型，把更多 layer/effect 级编辑压缩为更小增量，而不是回退到更粗粒度 rebuild。
- 把 modifier 从当前样式/compositor/effect/interaction 语义链继续扩展为可承载 filter graph、gesture arena、hover/pressed 生命周期、更完整 semantics tree 与交互状态的通用节点装饰模型。
- 在 `zeno-foundation` 中继续扩展 scroll 容器、basic controls 与交互基础组件，把当前 viewport 外壳推进到真正可消费 wheel/drag 输入的 `scroll`，并把更高层 design system 留给后续独立层。

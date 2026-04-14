# Rendering Architecture

## 状态
- 状态：已完成主链路 MVP
- 阶段判断：主渲染链路已经完成从“统一 session + scene/retained 过渡提交”到“`DisplayList` 单轨提交”的切换；Skia 与 macOS Impeller 都已具备原生 `DisplayList` renderer，`AppHost` 与 `RenderSession` 已不再保留旧提交分叉。

## 目标
- 让 Rust 保持框架控制平面。
- 在平台允许时优先走 Impeller 风格路径。
- 在 Impeller 未实现或不可用时回退到 Skia。
- 对上层暴露后端无关的渲染抽象。
- 在已完成的 `DisplayList` 单轨主链之上，继续收敛到下一代 `DisplayList + Compositor` 终极架构；详见 `display-list-compositor.md`。

## 分层
- `zeno-core` 负责共享类型、配置、平台标识与结构化错误。
- `zeno-text` 负责文本描述、布局契约、shaping 与 glyph/cache。
- `zeno-scene` 负责 `DisplayList`、`RetainedDisplayList`、`DrawCommand`、`LayerObject/RenderObject` 以及统一的 `Renderer` / `RenderSession` / backend probe 契约；runtime/platform/backend 边界已收口为 `DisplayList` 提交，旧 retained 提交类型已退出公开 API。
- `zeno-ui` 负责声明式节点树、modifier、retained tree、布局与从节点树到 `DisplayList` 输出的转换；内部仍使用 retained 数据面承接增量生成。
- `zeno-foundation` 负责 `text / container / row / column / spacer` 等首批基础构件。
- `zeno-runtime` 负责后端优先级、探测、回退选择、`UiRuntime`、帧调度以及平台无关的 `App/AppFrame/AppView/AppHost/run_app` 运行时模型。
- `zeno-platform` 负责宿主窗口、事件循环、surface 生命周期，以及桌面/移动端 render session 创建与 attachment；它只暴露 host/session 能力，不再承载 app 层入口。
- `zeno-backend-impeller` 与 `zeno-backend-skia` 负责具体后端实现。

## 当前渲染流程
1. Shell 根据当前平台生成 `NativeSurface` 与平台描述。
2. Platform session 层读取 `RendererConfig` 并生成后端尝试顺序。
3. 各 backend 根据平台执行 probe，返回可用性与失败原因。
4. Platform session 层基于 probe 结果生成统一 `ResolvedSession` descriptor。
5. Foundation/UI 层构建声明式节点树，`UiRuntime` 驱动 retained tree、布局与 patch/full 生成，并向外输出 `DisplayList` 与 patch 统计。当前 retained runtime 已完成 V2 对象表架构：`FrontendObjectTable` 是全链路索引与对象属性的唯一真相源，`DirtyTable`（bitset + generation）管理 style/intrinsic/layout/paint/scene/resource 六种脏类型，`LayoutWorkQueue` 以两阶段工作队列（intrinsic + placement）驱动布局而非递归。`NodeId` 仅在 frontend compile 阶段提供 keyed identity，运行时热路径全部通过稠密 `usize` index 驱动。
6. Runtime 的 `AppHost/run_app` 驱动 `App -> AppView -> UiRuntime -> RenderSession submit` 闭环，当前统一通过 `submit_display_list(...)` 提交。
7. 移动端在进入 render session 前，还会经过 `MobileAttachContext -> MobilePresenterInterface -> platform presenter builder` 的宿主绑定与 presenter 规划过程。

## 终极演进方向
- 当前主链路的数据协议仍然只把 `Scene(RenderObject/LayerObject)` 留作局部初始化/测试快照表达；正式运行时协议已经完全收敛到 `DisplayList`。真正的性能瓶颈已从“如何把 DisplayList 接进 backend”转移到“如何升级为 tile/compositor 级缓存与独立合成层”。
- 下一代目标协议是 `DisplayList + SpatialTree + ClipChain + StackingContext + DamageRegion + TileCache + CompositorLayerTree`。
- 终极目标链路将从“`Node -> FrontendObjectTable -> LayoutArena -> DisplayList -> Backend` 的当前单轨主链”，继续演进为 `Node -> FrontendObjectTable -> LayoutArena -> DisplayList -> Compositor -> Backend`。
- 在该目标架构中，Paint 只负责生成 `DisplayItem`，Rasterize 只负责重建脏 tiles，Composite 只负责 layer/effect/opacity/transform 合成。
- `transform / opacity / blend / effect` 类更新将优先下沉为 compositor-only 更新，而不是再引入任何 scene patch 兼容层。
- 该方案的完整设计、数据结构与阶段路线记录在 `display-list-compositor.md`。

## 已验证能力
- Workspace 已按 `core / graphics / runtime / shell / compose / foundation / text / backend-*` 垂直拆分。
- Runtime 已支持 Impeller 优先、Skia 兜底，并能记录每次解析尝试。
- `zeno-ui` 已具备 retained tree、dirty propagation、layout dirty roots 与局部 relayout 路径；keyed reorder 可下沉为 order patch，结构 dirty 会尽量归并到最小容器根，结构 patch 也不再因简单增删退化为整树 rebuild。
- retained runtime 已完成 V2 对象表架构重构：`FrontendObjectTable` 统一承载关系表、索引映射与对象快照，`DirtyTable`（bitset + generation）管理六种脏类型，`LayoutWorkQueue` 以两阶段工作队列驱动布局。`Node` 声明树仅作为 frontend compile 的输入，不进入运行时热路径。reconcile 已从 Node 树递归切换为 `FrontendObjectTable` 对象 diff；scene compiler、patch collect/diff/update、fragment 构建全部基于对象表显式栈遍历。
- `zeno-scene` 当前保留 `LayerObject`、`RenderObject`、`DrawCommand` 与 `Scene` 快照类型，用于 backend/test 辅助与初始化表达；`RetainedScene`、`RenderSceneUpdate` 等旧 retained 提交导出已经移除。
- `zeno-ui` 已具备 `RetainedDisplayList`、`SpatialTree`、`ClipChainStore`、`StackingContext` 与 `DisplayList` 快照能力；`TextRun` 已直接携带 `TextLayout` 数据，图片链路也已升级为 `ImageSource + ImageResourceTable + DisplayImage`，不再只是 cache key 占位或节点内联裸像素。
- `Renderer` / `RenderSession` / `AppHost` 已具备 `submit_display_list(...)` 单轨主提交流程；`RenderCapabilities::display_list_submit` 继续作为 backend probe 与平台能力矩阵字段保留。
- Skia 已具备原生 `DisplayList` renderer，支持基础图元、文本、图像、`ClipChain`、`SpatialTree`、`StackingContext`、blur 与 drop-shadow。
- macOS Impeller 已具备原生 `DisplayList` renderer，支持基础图元、文本、图像、`ClipChain` 交集裁剪、`StackingContext` 递归遍历与 offscreen 合成。
- `zeno-runtime` 已具备平台无关的 `App/AppFrame/AppView/AppHost + UiRuntime` 主干，并直接持有 `run_app` 入口；`zeno-platform` 只提供桌面/移动宿主与 session builder，外部也不再直接感知任何 retained/scene 提交协议。
- `UiRuntime` / `UiFrame` 的观测面也已经 display-list-first：examples、bench 与窗口 runtime 默认以 `display_list()` 和 frame report 作为主观测出口，而不是继续暴露 retained scene 提交细节。
- 移动端 shell 已具备 `session binding -> attachment -> presenter interface -> render session` 主链路。
- Android/iOS 已分别具备 native-window / view / metal-layer presenter builder，session 不再直接持有通用 renderer。

## 当前仍待补齐
- `DisplayList` 主链已经打通，但当前仍缺少真正独立的 `DamageTracker`、`TileGrid/TileCache`、`CompositorLayerTree` 与 compositor 线程；Skia 与 Impeller 目前仍各自执行原生 immediate/composite 风格后端遍历。
- 图片资源主链已具备最小可用模型，但仍缺少真正工程化的多来源与缓存层：当前已有 `ImageSource::Rgba8`、`ImageResourceKey`、`ImageResourceTable` 与 builder 解析入口，剩余重点是 asset/path/url 来源、解码缓存与失效/复用策略。
- 非 macOS 桌面 Impeller 路径仍未完成；macOS 上已具备 dirty-bounds 驱动的局部 GPU 提交闭环，剩余重点转向更复杂 filter graph、多级 effect 合成与进一步的缓存优化。
- 桌面按需调度目前已覆盖 pointer 输入与下一帧协商，但更高层的 lifecycle / visibility / gesture / keyboard 仍未与 `App/AppFrame + UiRuntime` 完整打通。
- 移动端 presenter 虽已成型，但 `ANativeWindow / UIView / CAMetalLayer` 到真实 swapchain、drawable、command buffer 生命周期的最后一跳仍未完全原生化。
- 文本主路径已补上 `TextSystem / TextShaper / TextCache` 抽象、glyph 级布局数据、fallback/system shaping 主干、Skia glyph-run 分段提交，以及由 `zeno-text` 持有的后端共享 glyph 栅格缓存；剩余重点转向更完整的 shaping 覆盖与更高阶缓存统计。
- 工程化验证能力已补上 `examples/text_probe`、统一单入口的 `examples/minimal_app`（内部可切换 physics / compose / compositor demo）、scene dump、layout dump、`examples/bench_gallery`、bench suite 脚本与 CI workflow；后续重点转向 golden image、基线管理与更复杂场景覆盖。
- 从架构终局看，当前不再需要解决“是否切换到 DisplayList 协议”，而是要继续补齐 `DamageTracker`、`TileGrid/TileCache`、独立 compositor 层与 compositor-only 动画路径。这部分不再是“继续细化 Scene patch”，而是把已落地的 `DisplayList` 协议继续升级成完整 compositor 架构。

## 为什么保持这种形状
- 选择器放在 runtime，可避免上层直接硬编码 Skia 或 Impeller。
- graphics API 可以在平台策略变化时尽量保持稳定。
- 文本系统独立存在，便于后续单独升级到真实 shaping 和缓存模型。
- shell 与 backend 分离，便于把桌面与移动端宿主能力统一收敛在同一平台集成层。

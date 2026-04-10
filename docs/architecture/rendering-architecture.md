# Rendering Architecture

## 状态
- 状态：已完成主链路 MVP
- 阶段判断：主渲染链路已经从“桌面双后端原型”推进到“统一 session + retained/patch 提交”阶段；桌面最成熟，移动端已经具备 attachment、platform presenter builder 与 render session 工厂链路。

## 目标
- 让 Rust 保持框架控制平面。
- 在平台允许时优先走 Impeller 风格路径。
- 在 Impeller 未实现或不可用时回退到 Skia。
- 对上层暴露后端无关的渲染抽象。

## 分层
- `zeno-core` 负责共享类型、配置、平台标识与结构化错误。
- `zeno-text` 负责文本描述、布局契约、shaping 与 glyph/cache。
- `zeno-scene` 负责 `Scene`、`DrawCommand`、`LayerObject/RenderObject`、`RenderObjectDelta/RenderSceneUpdate` 以及 `Renderer` trait 与 backend probe 契约。
- `zeno-ui` 负责声明式节点树、modifier、retained tree、布局与从节点树到 `RenderSceneUpdate` 的转换。
- `zeno-foundation` 负责 `text / container / row / column / spacer` 等首批基础构件。
- `zeno-runtime` 负责后端优先级、探测、回退选择、`UiRuntime`、帧调度以及平台无关的 `App/AppFrame/AppView/AppHost/run_app` 运行时模型。
- `zeno-platform` 负责宿主窗口、事件循环、surface 生命周期，以及桌面/移动端 render session 创建与 attachment；它只暴露 host/session 能力，不再承载 app 层入口。
- `zeno-backend-impeller` 与 `zeno-backend-skia` 负责具体后端实现。

## 当前渲染流程
1. Shell 根据当前平台生成 `NativeSurface` 与平台描述。
2. Platform session 层读取 `RendererConfig` 并生成后端尝试顺序。
3. 各 backend 根据平台执行 probe，返回可用性与失败原因。
4. Platform session 层基于 probe 结果生成统一 `ResolvedSession` descriptor。
5. Foundation/UI 层构建声明式节点树，`UiRuntime` 驱动 retained tree、布局与 patch/full 生成，输出后端无关的 `RenderSceneUpdate`。当前 retained runtime 已完成 V2 对象表架构：`FrontendObjectTable` 是全链路索引与对象属性的唯一真相源，`DirtyTable`（bitset + generation）管理 style/intrinsic/layout/paint/scene/resource 六种脏类型，`LayoutWorkQueue` 以两阶段工作队列（intrinsic + placement）驱动布局而非递归。`NodeId` 仅在 frontend compile 阶段提供 keyed identity，运行时热路径全部通过稠密 `usize` index 驱动。
6. Runtime 的 `AppHost/run_app` 驱动 `App -> AppView -> UiRuntime -> RenderSceneUpdate` 闭环，再调用 platform host 创建桌面或移动端 render session，并把 `RenderSceneUpdate` 提交给具体 GPU 或 Canvas 路径。
7. 移动端在进入 render session 前，还会经过 `MobileAttachContext -> MobilePresenterInterface -> platform presenter builder` 的宿主绑定与 presenter 规划过程。

## 已验证能力
- Workspace 已按 `core / graphics / runtime / shell / compose / foundation / text / backend-*` 垂直拆分。
- Runtime 已支持 Impeller 优先、Skia 兜底，并能记录每次解析尝试。
- `zeno-ui` 已具备 retained tree、dirty propagation、layout dirty roots 与局部 relayout 路径；keyed reorder 可下沉为 order patch，结构 dirty 会尽量归并到最小容器根，结构 patch 也不再因简单增删退化为整树 rebuild。
- retained runtime 已完成 V2 对象表架构重构：`FrontendObjectTable` 统一承载关系表、索引映射与对象快照，`DirtyTable`（bitset + generation）管理六种脏类型，`LayoutWorkQueue` 以两阶段工作队列驱动布局。`Node` 声明树仅作为 frontend compile 的输入，不进入运行时热路径。reconcile 已从 Node 树递归切换为 `FrontendObjectTable` 对象 diff；scene compiler、patch collect/diff/update、fragment 构建全部基于对象表显式栈遍历。
- `zeno-scene` 已具备 `LayerObject`、`RenderObject`、`RenderObjectDelta`、`RenderSceneUpdate` 数据结构，并支持 subtree clip / 2D affine transform / opacity / effect stack 状态。`Scene` 主体存储 `layer_graph/objects/packets`。
- Skia 已能消费结构化 scene，具备 dirty bounds 局部提交路径，并已执行 layer 级 blend / blur / drop-shadow MVP。
- macOS 已具备 Impeller Metal presenter，可走桌面窗口渲染路径，并已支持 layer 级 offscreen compositing、blend、blur 与 drop-shadow 执行链；patch 脏区已从根 pass 继续透传到 offscreen pass 的局部 scissor，partial scene 也会结合祖先 clip / offscreen / effect 链裁剪实际重放范围。
- `zeno-runtime` 已具备平台无关的 `App/AppFrame/AppView/AppHost + UiRuntime` 主干，并直接持有 `run_app` 入口；`zeno-platform` 只提供桌面/移动宿主与 session builder，外部不再直接感知 `RenderSceneUpdate + FrameRequest` 等内部提交/调度协议。
- 移动端 shell 已具备 `session binding -> attachment -> presenter interface -> render session` 主链路。
- Android/iOS 已分别具备 native-window / view / metal-layer presenter builder，session 不再直接持有通用 renderer。

## 当前仍待补齐
- `Scene` 已进入 retained compositor 的对象/packet 分层阶段；Skia 与 Impeller 都已落地 blend / blur / drop-shadow 执行链，当前主要待补的是更复杂 filter graph、多级 effect 优化与更细粒度 GPU patch。跨 crate 的 scene 协议已切至 `LayerObject/RenderObject`，`node_id` 只保留在必要的 debug/映射边界。
- 非 macOS 桌面 Impeller 路径仍未完成；macOS 上已具备 dirty-bounds 驱动的局部 GPU 提交闭环，剩余重点转向更复杂 filter graph、多级 effect 合成与进一步的缓存优化。
- 桌面按需调度目前已覆盖 pointer 输入与下一帧协商，但更高层的 lifecycle / visibility / gesture / keyboard 仍未与 `App/AppFrame + UiRuntime` 完整打通。
- 移动端 presenter 虽已成型，但 `ANativeWindow / UIView / CAMetalLayer` 到真实 swapchain、drawable、command buffer 生命周期的最后一跳仍未完全原生化。
- 文本主路径已补上 `TextSystem / TextShaper / TextCache` 抽象、glyph 级布局数据、fallback/system shaping 主干、Skia glyph-run 分段提交，以及由 `zeno-text` 持有的后端共享 glyph 栅格缓存；剩余重点转向更完整的 shaping 覆盖与更高阶缓存统计。
- 工程化验证能力已补上 `examples/text_probe`、统一单入口的 `examples/minimal_app`（内部可切换 physics / compose / compositor demo）、scene dump、layout dump、`examples/bench_gallery`、bench suite 脚本与 CI workflow；后续重点转向 golden image、基线管理与更复杂场景覆盖。

## 为什么保持这种形状
- 选择器放在 runtime，可避免上层直接硬编码 Skia 或 Impeller。
- graphics API 可以在平台策略变化时尽量保持稳定。
- 文本系统独立存在，便于后续单独升级到真实 shaping 和缓存模型。
- shell 与 backend 分离，便于把桌面与移动端宿主能力统一收敛在同一平台集成层。

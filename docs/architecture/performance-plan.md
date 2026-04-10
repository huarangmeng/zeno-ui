# Performance And DX Plan

## 状态
- 状态：进行中
- 目标：把当前“桌面双后端原型”演进成适合 Compose 风格跨平台 UI 的高性能架构，同时提升调试、验证与接入体验。
- 当前完成度：P0 已完成；P1 已完成 retained tree、layout dirty roots 与 Structured Scene 的 MVP 主链路；P2 中“文本主路径 + bench/devtools 工程化能力”已完成，后续重点转向更复杂 effect/filter、golden image 与移动端 presenter 原生化。

## 当前阶段判断
- 当前主链路已经成立：`zeno-ui -> zeno-scene::Scene -> zeno-runtime -> zeno-platform -> backend-*`。
- 当前最大的收益点不在继续堆更多组件或绘制命令，而在补齐 retained tree、帧调度、缓存与统一的渲染会话抽象。
- 桌面是当前最成熟的验证面：Skia 可用，macOS Impeller 有 Metal presenter 原型。

## 当前瓶颈

### 1. 局部更新能力已具备 MVP，仍待继续细化
- `zeno-ui` 已具备 retained tree、节点 dirty、layout dirty roots 与局部 relayout 路径；当前 runtime 数据面已进一步收敛为 `NodeIndexTable + LayoutArena + DenseNodeStore + FragmentStore`，dirty/fragment/patch 主路径改为 index-first。
- `Scene` 已从单纯扁平命令流升级到 block/patch 提交模型，session 可消费 `SceneSubmit`。
- 当前剩余差距主要在更细粒度的 dirty root 归并、更复杂结构编辑下的 patch 收敛，以及后端更深层级的局部 GPU 提交能力。

### 2. 按需重绘主链路已完成，仍待继续细化动画与 invalidation 策略
- 桌面事件循环已经从空闲态持续 redraw 切换为按需驱动。
- 当前剩余问题主要在动画驱动、未来更细粒度 invalidation 与观测工具，而不是空闲态自旋本身。

### 3. runtime 与 shell 边界已完成收敛
- runtime 现在聚焦 `App/AppFrame/AppHost/run_app + UiRuntime` 闭环，不再持有平台 session descriptor 的真相源。
- `ResolvedSession` 已统一收敛到 `zeno-platform`，作为平台 host/session 层的单一 descriptor；具体桌面/移动端 `RenderSession` 创建也继续保留在 `zeno-platform`。
- 当前剩余工作主要是把移动端已成型的 presenter builder 继续推进到真实 GPU 生命周期，而不是拆成多个平台专用 crate。

### 4. Scene 已完成第一阶段结构化，第二阶段仍待推进
- 当前 `Scene` 已具备 `SceneBlock`、`ScenePatch`、`SceneSubmit`，不再只是单纯扁平命令流。
- 当前剩余差距主要是 layer、clip、transform、更强的资源句柄化与更缓存友好的结构。

### 5. 文本系统主路径已打通，仍待继续做强
- `zeno-text` 已具备 fallback/system 双路径、paragraph cache、共享 glyph raster cache 与真实 shaping 主干。
- 上层布局、Skia glyph-run 提交与 Impeller glyph 栅格缓存已经对齐，剩余工作主要是更完整的 shaping 覆盖、更多缓存统计与更强的字体/脚本支持。

## 目标架构

### Retained UI Tree
- 为 `zeno-ui` 引入稳定 `NodeId`，并在 runtime 内部收敛到统一 `NodeIndexTable`。
- 让 UI 树保留上一帧结构、测量结果和局部 dirty 信息。
- 把“全量重建”演进为“dirty subtree 更新”。

### Render Session
- 让 runtime 的解析结果直接变成可驱动窗口呈现的统一 session。
- 上层不再显式关心 backend 分支，只关心提交帧、resize、能力与统计。

### Structured Scene
- 逐步把 `Scene` 从扁平命令流升级为结构化提交模型。
- 第一阶段先引入资源键与缓存友好结构。
- 第二阶段再补 layer、clip、transform 与局部节点块。

### Text Pipeline
- 将 `FallbackTextSystem` 明确降级为兜底实现。
- 主路径逐步引入真实 shaping、paragraph cache 与 glyph cache。

### Frame Scheduler
- 将桌面事件循环从持续重绘改为按需重绘。
- 显式区分 `needs_layout`、`needs_paint`、`needs_present`。

## 推荐执行顺序

### P0：收敛抽象边界
- 状态：已完成
- 已完成 `ResolvedSession -> RenderSession` 链路，runtime 保持 app/runtime 调度职责，platform 作为单一平台集成层负责 descriptor 与具体会话创建。
- 已移除 shell 内“按 backend 二次分发再决定谁负责”的旧模式，统一入口现在是 `ResolvedSession` + 平台集成工厂。
- 移动端已进一步收敛为 `binding -> attachment -> presenter interface -> platform presenter builder -> render session` 单链路。

### P0：改造帧调度
- 去掉空闲态持续 redraw。
- 用 invalidate 驱动下一帧，而不是让事件循环持续自旋。

### P1：引入 retained tree 与 dirty propagation
- 状态：已完成（MVP）
- 已完成稳定 `NodeId`、retained tree、dirty propagation、paint-only 快路径。
- 已完成 layout dirty roots 与局部 relayout 主链路，小范围 layout 更新不再必然退化为全树测量。

### P1：升级 Scene 结构
- 状态：已完成（MVP）
- 已完成 `SceneBlock`、`ScenePatch`、`SceneSubmit` 主数据结构，并打通 compose/runtime/shell/session 提交流。
- 已完成 block 统计、patch upserts/removes 统计与 session 侧 patch 消费入口。

### P2：升级文本系统
- 状态：已完成（主路径）
- 已统一布局、绘制与缓存的文本数据模型。
- 已为 Skia 与 Impeller 接入真实缓存能力：Skia glyph-run 分段提交、Impeller 共享 glyph raster cache、system shaping 与 paragraph cache 已接通。

### P2：补工程化体验
- 提供 bench gallery、layout dump、scene dump、frame stats。
- 状态：已完成（当前阶段目标）
- 已提供根 crate 级平台 preset feature：`macos`、`linux`、`windows`、`android`、`ios`。
- 已提供 `examples/text_probe`、`examples/bench_gallery`、bench suite 脚本与 `perf-regression` workflow。
- 剩余工作聚焦在 golden image、性能基线管理与更多场景覆盖。

## 对各 crate 的具体建议

### zeno-ui
- 引入 `NodeId`、diff、dirty propagation、布局缓存。
- 把 `ComposeRenderer` 从“单次函数式翻译器”演进为“可保留上下文的 compose engine”。

### zeno-scene
- 保持 `DrawCommand` 的简单性，但逐步补充资源句柄和更适合后端缓存的数据结构。
- 给 renderer/session 层预留统一 frame report 入口。

### zeno-runtime
- 继续保留 backend probe/fallback 逻辑。
- 让 `ResolvedSession` 继续承担统一 descriptor 角色，并把平台、attempts 与调试元数据稳定沉淀在这一层。

### zeno-platform
- 保持 shell 只负责窗口、surface、事件循环和宿主对象。
- 不让后端渲染逻辑重新回流到 shell 内部。

### zeno-text
- 拆出更明确的 text system / shaper / cache 边界。
- 让文本布局结果可以被共享和缓存，而不是每次测量后仅作为一次性数据使用。

### zeno-backend-skia / zeno-backend-impeller
- 都以统一 session 和统一 scene 提交模型为目标。
- 先做缓存和统计，再扩更复杂效果。

## 开发体验建议

### 更清晰的 feature 预设
- 核心库默认保持轻量。
- 已通过根 crate 级 preset feature 提供更直接的平台入口：`macos`、`linux`、`windows`、`android`、`ios`。
- 同时保留 `desktop`、`mobile_android`、`mobile_ios` 作为更底层的能力 feature。

### 更可重复的验证手段
- 增加 benchmark 示例，而不是只依赖最小 demo。
- 为深树布局、长文本、多次小更新建立基准场景。

### 更可观测的调试工具
- 输出 backend attempts。
- 增加 frame stats。
- 增加 scene dump 与 layout dump。

## 完成标准
- 小范围 UI 更新不再触发全树布局与全量 Scene 重建。
- 空闲态不持续重绘。
- runtime 与 shell 的渲染边界统一。
- 文本布局与渲染开始共享缓存体系。
- 新开发者可以通过 preset feature 与 benchmark 场景快速理解系统行为。

## 当前已完成项
- `ResolvedSession` 已成为统一 session descriptor，平台集成层可基于它创建具体桌面/移动端 `RenderSession`。
- `UiRuntime` 已成为内部重绘决策与 frame 准备入口，对上层隐藏 `ComposeEngine`。
- `FrameScheduler` 已将桌面空闲态持续 redraw 改为按需重绘。
- `RetainedComposeTree` 已具备稳定 `NodeId` identity、index-first dirty propagation、index-first layout dirty roots 与局部 relayout 主链路。
- `Scene` 已具备 `SceneBlock` / `ScenePatch` / `SceneSubmit`，桌面 session 已按结构化提交模型消费场景。
- `SkiaTextCache` 已具备 typeface/font 缓存与命中统计。
- 帧统计已输出 `block_count`、`patch_upserts`、`patch_removes`，可直接观察增量提交行为。
- 根 crate 已提供 `macos`、`linux`、`windows`、`android`、`ios` 平台 preset feature，降低首次接入成本。
- 移动端已固定 `MobilePresenterInterface`，并为 Android/iOS 建立 platform presenter builder 与 renderer-backed session 适配层。
- `Scene` 已进入 layer/block 结构化阶段，并补上 blend / effect / offscreen 等高阶抽象。
- `zeno-text` 已拆出 `TextShaper / TextCache` 抽象，fallback 路径具备 paragraph cache 与命中统计。
- `zeno-text` 已补 `SystemTextShaper / SystemTextSystem`、共享 glyph raster cache、Skia glyph-run 分段提交与后端共享缓存入口。
- 已补 `examples/text_probe`、`examples/bench_gallery`、bench suite 脚本与 `perf-regression` workflow，可输出机读指标并在阈值回归失败时阻断。

## 当前未完成项
- layout dirty 已收敛到更小祖先集合，并新增同父结构/顺序脏根合并与最小容器根策略；后续重点转向更复杂 layer/effect tree 下的 patch 类型扩展，而不是继续放大祖先影响面。
- `Scene` 已具备 layer/clip/transform/blend/effect/offscreen 抽象，macOS Impeller 也已支持根 pass / offscreen pass 的双层 scissor 与祖先 clip 链约束下的 partial scene 重放；下一阶段重点是更细粒度 compositor 与后端 effect 合成。
- Skia 与 macOS Impeller 都已具备 dirty bounds 局部提交路径；当前待补齐的是非 macOS Impeller presenter、更多 effect/filter 组合以及更稳定的缓存与统计体系。
- 文本主路径已具备 `TextSystem / TextShaper / TextCache` 主干、glyph 级布局数据、Skia glyph-run 分段提交与后端共享缓存；后续重点是更完整的 shaping 覆盖、字体 fallback 策略与更细粒度缓存统计。
- scene dump、layout dump、text probe、bench gallery 与自动化 bench suite 已就位；后续重点是 golden image、基线管理与更多 DX 工具。

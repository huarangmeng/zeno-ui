# Performance And DX Plan

## 状态
- 状态：进行中
- 目标：把当前“桌面双后端原型”演进成适合 Compose 风格跨平台 UI 的高性能架构，同时提升调试、验证与接入体验。
- 当前完成度：P0 基本完成；P1 已完成 retained/dirty 与局部重发射基础；P2 仍以文本系统与工程化能力为主。

## 当前阶段判断
- 当前主链路已经成立：`zeno-compose -> zeno-graphics::Scene -> zeno-runtime -> zeno-shell -> backend-*`。
- 当前最大的收益点不在继续堆更多组件或绘制命令，而在补齐 retained tree、帧调度、缓存与统一的渲染会话抽象。
- 桌面是当前最成熟的验证面：Skia 可用，macOS Impeller 有 Metal presenter 原型。

## 当前瓶颈

### 1. 全树布局与全量 Scene 生成
- `zeno-compose` 目前仍采用即时模型：节点树每次都重新测量并生成完整 `Scene`。
- 这意味着小改动也会触发全树 `measure_node` 与全量 `emit_node`。

### 2. 持续重绘
- 桌面事件循环仍会在空闲阶段持续请求 redraw。
- 这种模型对 CPU 占用、功耗与未来动画调度都不友好。

### 3. runtime 与 shell 边界收敛中
- runtime 已负责解析 backend，并直接生成 `ResolvedSession`。
- `ResolvedSession` 现在作为纯 descriptor 保留在 runtime；具体桌面 `RenderSession` 创建已统一收敛到 `zeno-shell` 这一平台集成层。
- 当前剩余工作主要是继续把移动端 presenter 能力接入同一平台集成 crate，而不是拆成多个平台专用 crate。

### 4. Scene 模型过于扁平
- 当前 `Scene` 本质上是 `Vec<DrawCommand>`。
- 这足够验证“能画出来”，但不利于局部重绘、批处理、资源句柄化和后端缓存。

### 5. 文本系统仍偏占位
- 当前 `zeno-text` 仍以 fallback 测量为主。
- 上层布局、下层真实绘制与未来 shaping/cache 之间尚未统一。

## 目标架构

### Retained UI Tree
- 为 `zeno-compose` 引入稳定 `NodeId`。
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
- 已完成 `ResolvedSession -> RenderSession` 链路，runtime 保持解析与调度职责，shell 作为单一平台集成层负责具体会话创建。
- 已移除 shell 内“按 backend 二次分发再决定谁负责”的旧模式，统一入口现在是 `ResolvedSession` + 平台集成工厂。

### P0：改造帧调度
- 去掉空闲态持续 redraw。
- 用 invalidate 驱动下一帧，而不是让事件循环持续自旋。

### P1：引入 retained tree 与 dirty propagation
- 为 `zeno-compose` 增加稳定节点标识。
- 为布局和 Scene 生成增加缓存与失效传播。

### P1：升级 Scene 结构
- 引入资源键、可缓存文本布局、路径/笔刷复用入口。
- 为后端批处理保留结构空间。

### P2：升级文本系统
- 统一布局、绘制与缓存的文本数据模型。
- 为 Skia 与 Impeller 分别接入真实缓存能力。

### P2：补工程化体验
- 提供 bench gallery、layout dump、scene dump、frame stats。
- 提供更直接的平台 preset feature，减少首次接入成本。

## 对各 crate 的具体建议

### zeno-compose
- 引入 `NodeId`、diff、dirty propagation、布局缓存。
- 把 `ComposeRenderer` 从“单次函数式翻译器”演进为“可保留上下文的 compose engine”。

### zeno-graphics
- 保持 `DrawCommand` 的简单性，但逐步补充资源句柄和更适合后端缓存的数据结构。
- 给 renderer/session 层预留统一 frame report 入口。

### zeno-runtime
- 继续保留 backend probe/fallback 逻辑。
- 但把当前 `ResolvedRenderer` 演进为更完整的 resolved session 或 resolved presenter。

### zeno-shell
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
- 通过 workspace 级 preset feature 提供更直接的体验，如桌面 demo、桌面 auto backend 等。

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
- `ResolvedSession` 已成为统一 session descriptor，平台集成层可基于它创建具体桌面 `RenderSession`。
- `UiRuntime` 已成为内部重绘决策与 frame 准备入口，对上层隐藏 `ComposeEngine`。
- `FrameScheduler` 已将桌面空闲态持续 redraw 改为按需重绘。
- `SkiaTextCache` 已具备 typeface/font 缓存与命中统计。

## 当前未完成项
- layout dirty 仍未做到真正脏子树局部重测量。
- `Scene` 仍是扁平命令流，尚未演进到 layer/clip/transform/局部块提交模型。
- 文本主路径仍是 fallback 测量，真实 shaping / glyph cache / paragraph cache 尚未接入。
- bench gallery、scene dump、layout dump 等工程化工具仍未完成。

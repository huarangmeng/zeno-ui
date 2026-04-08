# Zeno UI 架构诊断与性能 / 开发体验优化计划

## Summary
- 目标：基于当前 `Compose -> Scene -> Runtime -> Shell -> Backend` 主链路，制定一份面向“真正全平台”的优化路线图，优先提升渲染性能，同时兼顾开发体验。
- 结论：当前分层方向是对的，但实现仍停留在“可跑通骨架”阶段；最大的性能瓶颈是全树布局 / 全量 Scene 生成 / 连续重绘 / 文本能力薄弱，最大的体验瓶颈是运行时抽象重复、feature 组合复杂、调试与验证能力缺失。
- 建议总方向：从“即时全量重建”逐步演进到“保留树 + 脏区驱动 + 帧调度 + 资源缓存”的管线；同时把 `runtime`、`shell`、`backend` 的职责边界进一步收敛，让上层只面向统一的提交接口。

## Current State Analysis

### 已确认的架构现状
- 工作区按职责拆分为 `zeno-core`、`zeno-text`、`zeno-graphics`、`zeno-compose`、`zeno-runtime`、`zeno-shell`、`zeno-backend-*`，整体分层清晰，见 `Cargo.toml`、`docs/architecture/rendering-architecture.md`。
- `zeno-compose` 已能将声明式节点树转成 `Scene`，核心入口在 `crates/zeno-compose/src/render.rs`。
- `zeno-runtime` 已实现 “Impeller 优先，Skia 兜底” 的解析顺序，但返回值仍停留在 `ResolvedRenderer { backend_kind, renderer }`，未形成统一的提交闭环，见 `crates/zeno-runtime/src/resolver.rs`。
- `zeno-shell` 已打通 winit + glutin + Skia 的桌面渲染链路，但 Impeller presenter 仍是 scaffold，见 `crates/zeno-shell/src/window.rs`。
- `zeno-text` 仍是 fallback 估算式文本布局，尚未具备真实 shaping / glyph cache / 平台字体能力，见 `crates/zeno-text/src/system.rs`。

### 当前性能问题
- `measure_node` 对整棵树递归测量，任何更新都会走全树布局，热点集中在 `crates/zeno-compose/src/layout.rs`。
- `emit_node` 每次都从测量结果全量发射 `Scene`，没有局部 diff、节点缓存与绘制批处理，见 `crates/zeno-compose/src/render.rs`。
- 文本命令在生成阶段发生 `layout.clone()`，高文本密度场景会引入额外复制成本，见 `crates/zeno-compose/src/render.rs`。
- `about_to_wait` 无条件 `request_redraw()`，当前是持续重绘模型，空闲帧也会消耗 CPU / GPU，见 `crates/zeno-shell/src/window.rs`。
- Skia 路径每帧都会重新 wrap backend render target、flush、swap，缺少更长期的 surface / resource 生命周期管理，见 `crates/zeno-shell/src/window.rs`。

### 当前开发体验问题
- `runtime` 负责选后端，但 `shell` 仍依据 `backend_kind` 再分支 presenter，存在双重分发与抽象重复。
- `desktop_winit`、`real_skia` 等 feature 组合对新使用者不够友好，缺少按平台分组的推荐 preset。
- 缺少面向 UI 框架的调试设施：布局树检查、重绘原因追踪、帧耗时拆分、后端 probe 可视化。
- 缺少稳定的性能基线：没有 benchmark 场景、没有 golden scene / golden image、没有跨 backend 一致性验证。

## Proposed Changes

### 1. 将 `zeno-compose` 从“即时全量构建”演进为“保留树 + 脏标记”

**目标文件**
- `crates/zeno-compose/src/node.rs`
- `crates/zeno-compose/src/layout.rs`
- `crates/zeno-compose/src/render.rs`
- `crates/zeno-compose/src/lib.rs`
- 建议新增：
  - `crates/zeno-compose/src/tree.rs`
  - `crates/zeno-compose/src/invalidation.rs`
  - `crates/zeno-compose/src/diff.rs`

**要做什么**
- 引入稳定节点标识、保留式 UI 树、脏标记传播机制。
- 将布局和 Scene 生成从“每帧全量递归”调整为“仅计算 dirty subtree”。
- 让文本布局结果、节点测量结果、绘制片段可被节点级缓存复用。

**为什么**
- 这是性能收益最大的核心改造，能直接降低布局、Scene 构建、文本测量的重复成本。
- 它也是未来动画、输入、焦点、命中测试、可访问性系统的基础。

**怎么做**
- 在 `node.rs` 中为节点引入稳定 `NodeId` 与更明确的结构化树表示。
- 在 `layout.rs` 中拆分 `measure_node` 为“输入收集 / 约束传递 / 布局缓存命中 / 脏子树刷新”几个阶段。
- 在 `render.rs` 中让 `emit_node` 面向缓存后的布局结果工作，避免每次从零遍历所有静态节点。
- 新增 `diff.rs` 负责比较上一帧与当前 declarative tree 的结构变化；新增 `invalidation.rs` 负责 style / text / geometry / children 层级的 dirty reason 传播。

### 2. 收敛 Runtime / Shell / Backend 边界，统一渲染提交接口

**目标文件**
- `crates/zeno-runtime/src/resolver.rs`
- `crates/zeno-runtime/src/lib.rs`
- `crates/zeno-graphics/src/renderer.rs`
- `crates/zeno-graphics/src/surface.rs`
- `crates/zeno-shell/src/shell.rs`
- `crates/zeno-shell/src/window.rs`

**要做什么**
- 将当前 “runtime 选 backend + shell 再按 backend_kind 决定 presenter” 的双重分发，收敛成单一渲染会话接口。
- 让 `runtime` 产出的不只是 `Renderer`，而是“绑定具体 surface / presenter 能力的 session 或 executor”。

**为什么**
- 当前职责边界让上层知道太多后端细节，不利于扩展到 iOS / Android，也会让 Impeller 真正接入时出现更多分支。
- 统一会话接口后，上层可只关心 `submit(scene)`、`resize()`、`present()`、`capabilities()`。

**怎么做**
- 在 `zeno-graphics` 中定义更完整的 surface / frame session 抽象。
- 在 `resolver.rs` 中把“探测 + 构建 renderer + 绑定 surface”收敛成一条链。
- 在 `window.rs` 中去掉按 `BackendKind` 直接分支的高层逻辑，让 presenter 成为 backend 内部能力的一部分，shell 只管理窗口生命周期与输入事件。
- 后续 Impeller、Skia 都实现同一套 frame session 协议，便于统一 profiling 与测试。

### 3. 为 Scene 层引入批处理、资源缓存与更细粒度命令模型

**目标文件**
- `crates/zeno-graphics/src/scene.rs`
- `crates/zeno-graphics/src/renderer.rs`
- `crates/zeno-backend-skia/src/lib.rs`
- `crates/zeno-backend-impeller/src/lib.rs`

**要做什么**
- 将当前偏“立即执行式”的 `DrawCommand` 扩展为更利于 backend 批处理的命令模型。
- 为笔刷、路径、文本布局、图像 / 纹理句柄建立缓存键。
- 预留分层 Scene / RenderPass / Clip / Transform 栈能力，而不是所有命令都扁平落在一个列表里。

**为什么**
- 后端性能优化最终依赖可分析的提交模型；如果 Scene 语义过浅，Skia 和 Impeller 都只能被动逐条解释。
- 批处理与缓存能减少状态切换、重复 shape 构建与重复文本栅格化。

**怎么做**
- 在 `scene.rs` 为命令补充稳定资源引用与分层结构，避免大对象频繁 clone。
- 在 `renderer.rs` 中加入资源缓存与帧内统计接口，为后续 devtools 提供数据。
- 在 `zeno-backend-skia` 先实现最小资源缓存闭环，再把相同抽象复制到 Impeller 实现。

### 4. 重构文本系统：从 fallback 测量升级为可插拔 shaping 管线

**目标文件**
- `crates/zeno-text/src/system.rs`
- `crates/zeno-text/src/types.rs`
- `crates/zeno-text/src/lib.rs`
- `crates/zeno-backend-skia/src/lib.rs`
- 建议新增：
  - `crates/zeno-text/src/cache.rs`
  - `crates/zeno-text/src/shaper.rs`

**要做什么**
- 将 `FallbackTextSystem` 明确降级为测试 / 无平台能力时的兜底实现。
- 抽象文本 shaping、font fallback、glyph cache、段落布局缓存能力。
- 文本布局结果改为引用式或共享式结构，避免在 `Scene` 发射阶段深拷贝。

**为什么**
- 文本通常是 UI 框架最重的性能路径之一，也是跨平台一致性与开发体验的关键。
- 没有真实文本系统，就很难做复杂控件、国际化、基线对齐与可测的布局结果。

**怎么做**
- `system.rs` 拆成 `TextSystem`、`TextShaper`、`TextCache` 三层接口。
- `types.rs` 引入更稳定的 paragraph key / glyph run / line metrics 数据模型。
- 优先让 Skia 路径接入真实 shaping 与缓存接口；其它平台先适配同一抽象。

### 5. 把帧循环从“持续重绘”改为“事件驱动 + 脏区调度”

**目标文件**
- `crates/zeno-shell/src/window.rs`
- `crates/zeno-shell/src/shell.rs`
- `crates/zeno-core/src/config.rs`
- 建议新增：
  - `crates/zeno-runtime/src/frame_scheduler.rs`

**要做什么**
- 取消空闲时无条件 `request_redraw()` 的策略，改为基于输入、动画、定时器、资源准备完成、后端回调等信号触发绘制。
- 引入 `needs_layout`、`needs_paint`、`needs_present` 等帧阶段状态。
- 为未来动画留出可配置的 frame pacing 和 vsync 策略。

**为什么**
- 对跨平台 UI 库来说，帧调度模型直接影响 CPU 占用、功耗与输入延迟。
- 空闲不渲染是桌面与移动共同需要的基础策略。

**怎么做**
- 在 `window.rs` 中将 redraw 请求改为条件触发。
- 在 `frame_scheduler.rs` 中集中管理“谁触发下一帧、何时只做 layout / paint / present 中的一部分”。
- 在 `config.rs` 中增加调试开关，例如强制连续绘制、启用帧统计、禁用局部重绘，方便 benchmark 对比。

### 6. 先补“工程化体验层”，再继续扩平台

**目标文件**
- `Cargo.toml`
- `crates/zeno-shell/Cargo.toml`
- `examples/minimal_app/src/main.rs`
- `docs/architecture/backend-selection.md`
- `docs/architecture/roadmap.md`
- 建议新增：
  - `docs/architecture/performance-plan.md`
  - `docs/architecture/devtools-plan.md`
  - `examples/bench_gallery/`

**要做什么**
- 提供按平台 / 目标场景分组的 feature preset，例如 `desktop_skia_demo`、`desktop_auto_backend`、`mobile_shell_stub`。
- 提供 benchmark 示例工程，而不是只保留最小演示。
- 为调试建立统一输出面：backend probe、frame stats、layout tree dump、scene dump。

**为什么**
- 真实开发体验不仅是 API 好不好用，还包括“第一次跑通难不难、出了问题怎么定位、改完如何验证”。
- 在真正推进全平台前，先把验证与观测能力补上，可以避免后续反复返工。

**怎么做**
- 整理 workspace feature 入口，减少使用者对 crate 内部 feature 的感知。
- 在 `examples` 中增加稳定 benchmark 场景：长文本、深树布局、滚动列表、透明叠层。
- 将文档从“架构说明”补到“为什么这样设计 + 如何验证优化是否生效”。

## Assumptions & Decisions
- 本计划以“真正全平台”为目标，但阶段落地上仍建议先把通用抽象和桌面验证链路做扎实，再复制到移动端实现。
- 性能优先意味着：先优化 `compose / text / scene / frame scheduling` 的核心成本，再做更多上层 widget 语义扩展。
- 不建议把大量宿主逻辑塞回 `zeno-shell`；应继续保持 shell 只负责平台生命周期，backend 负责具体 GPU / text 实现。
- 不建议继续沿着“每帧完整构造 Scene”的方向堆功能；如果不先引入保留树与脏标记，后续复杂控件和动画会迅速放大成本。
- Impeller 的优先级仍然保留，但工程顺序上应先完成统一抽象，再把 Impeller 接到同一渲染会话协议里，否则会产生第二套专用路径。

## Recommended Execution Order
1. `zeno-compose`：NodeId、dirty propagation、布局缓存。
2. `zeno-shell` + `zeno-runtime`：事件驱动帧调度、去掉持续重绘。
3. `zeno-graphics`：Scene 资源键、批处理友好命令模型。
4. `zeno-text`：真实 shaping 抽象与布局缓存。
5. `zeno-backend-skia`：先实现资源缓存与统一 frame session。
6. `zeno-backend-impeller`：在统一抽象上补真实 presenter，而不是单独生长。
7. `docs` + `examples`：基准场景、调试工具、feature preset。

## Verification Steps
- 结构验证：
  - `cargo check --workspace`
  - `cargo test --workspace`
- 功能验证：
  - 运行最小示例，确认 Auto / PreferSkia / PreferImpeller 三种配置的后端选择与回退行为符合预期。
  - 验证窗口空闲时不再持续重绘，输入或动画触发时才请求下一帧。
- 性能验证：
  - 基于新增 benchmark 场景记录布局耗时、文本布局耗时、Scene 构建耗时、后端提交耗时。
  - 对比“全量重建”与“dirty subtree”方案在深树 / 长文本 / 高频更新场景下的 CPU 占用。
  - 对比启用 / 禁用文本缓存、Scene 资源缓存、局部重绘时的帧时间变化。
- 一致性验证：
  - 为同一 `Scene` 在 Skia 与 Impeller 上建立 golden scene / image 校验。
  - 为 layout tree 与 scene dump 建立稳定输出测试，保证重构不破坏上层语义。

## Success Criteria
- 空闲态无持续重绘，CPU 占用显著下降。
- 非结构性小改动只触发局部布局与局部 Scene 更新，而不是全树重建。
- 文本布局与绘制具备缓存能力，文本密集场景帧耗时明显下降。
- 上层 API 不需要感知具体 backend / presenter 分支。
- 新开发者能通过简化后的 feature preset 与 benchmark / dump 工具快速理解并验证系统行为。

# Zeno UI（Compose 风格跨平台 UI）架构与性能/体验提升计划

## Summary
- 目标：在现有 `zeno-compose -> zeno-graphics(Scene) -> zeno-runtime(backend resolve) -> zeno-shell(presenter) -> backend-*` 主链路基础上，制定一套“性能优先、真正全平台”的落地路线图，同时提升开发体验与可维护性。
- 核心判断：当前仓库已经从“骨架期”进入“桌面双后端原型期”（Skia 可用、macOS Impeller 有 Metal presenter 原型），但还缺少 UI 框架进入高性能阶段必须具备的 retained tree、脏区/帧调度、资源与文本缓存、统一 render session 边界。
- 总策略：优先做“抽象收敛 + 帧调度 + retained/diff + 缓存/批处理 + 文本体系”，再扩展更复杂 UI 能力与移动端宿主实现。

## Current State Analysis（基于仓库现状的事实）

### Workspace 分层已成立
- Workspace 成员：根 crate + `crates/zeno-*` + `examples/minimal_app`，见 [Cargo.toml](file:///Users/bytedance/RustroverProjects/zeno-ui/Cargo.toml#L1-L13)。
- 总体分层说明：见 [rendering-architecture.md](file:///Users/bytedance/RustroverProjects/zeno-ui/docs/architecture/rendering-architecture.md)。

### 渲染主链路已打通（但边界仍不够统一）
- Compose：节点树全量测量并发射 `Scene`，见 [layout.rs](file:///Users/bytedance/RustroverProjects/zeno-ui/crates/zeno-compose/src/layout.rs#L20-L168) 与 [render.rs](file:///Users/bytedance/RustroverProjects/zeno-ui/crates/zeno-compose/src/render.rs#L20-L73)。
- Graphics：Scene 为扁平命令流 `Vec<DrawCommand>`，见 [scene.rs](file:///Users/bytedance/RustroverProjects/zeno-ui/crates/zeno-graphics/src/scene.rs#L22-L48)。
- Runtime：后端解析 Impeller 优先、Skia 兜底，见 [resolver.rs](file:///Users/bytedance/RustroverProjects/zeno-ui/crates/zeno-runtime/src/resolver.rs#L33-L109)。
- Shell：桌面窗口与 presenter 分发点 `DesktopGpuPresenter`，见 [window.rs](file:///Users/bytedance/RustroverProjects/zeno-ui/crates/zeno-shell/src/window.rs#L208-L253)。

### 已知性能与体验短板（当前最影响“像 Compose 一样可扩展”）
- 全树布局与全量 Scene：任意变更都会触发全树 `measure_node` 与全量 `emit_node`。
- 持续重绘：`about_to_wait` 会持续请求 redraw（空闲耗 CPU/GPU），见 [window.rs](file:///Users/bytedance/RustroverProjects/zeno-ui/crates/zeno-shell/src/window.rs#L180-L184)。
- 文本系统仍是 fallback 测量模型，见 [system.rs](file:///Users/bytedance/RustroverProjects/zeno-ui/crates/zeno-text/src/system.rs#L3-L49)。
- runtime 与 shell 边界重复：runtime 返回 renderer，但示例与 shell 路径仍以 `backend_kind` 做二次分发，见 [main.rs](file:///Users/bytedance/RustroverProjects/zeno-ui/examples/minimal_app/src/main.rs#L31-L63)。

### 文档现状
- `docs/architecture` 已整理出一份总览索引与每篇状态定义，见 [README.md](file:///Users/bytedance/RustroverProjects/zeno-ui/docs/architecture/README.md)。

## Proposed Changes（决策完整的落地方案）

### 1) 收敛 Runtime / Shell / Backend：引入统一 RenderSession（最高优先级）
**目标**
- 消除“runtime 解析 backend”和“shell 再按 backend 分发 presenter”的双重抽象。
- 上层只面对统一会话：`begin_frame/submit(scene)/present/resize/capabilities/frame_report`。

**涉及文件（预期改动点）**
- `crates/zeno-graphics/src/renderer.rs`：从单次 `render(&RenderSurface, &Scene)` 演进到会话/帧接口（可保留兼容层）。
- `crates/zeno-runtime/src/resolver.rs`：解析结果从 `ResolvedRenderer` 升级为 `ResolvedSession`（或 `ResolvedPresenter`），负责“探测 + 创建 + 绑定 surface”闭环。
- `crates/zeno-shell/src/window.rs`：桌面 presenter 不再由 shell 直接按 `Backend` 分支解释，改为由 session 持有/驱动。
- `crates/zeno-shell/src/shell.rs`：强化 `NativeSurface` 描述能力（可选携带原生句柄/Metal layer/swapchain 线索），避免仅做“逻辑尺寸”。

**验收标准**
- `minimal_app` 不再需要显式按 `backend_kind` 分支调用不同窗口路径；同一套调用可驱动 Skia/Impeller。
- `Resolved*` 结构能输出“为何选择/为何回退”的 attempts，并能输出稳定的帧统计入口。

### 2) 帧调度从持续重绘改为按需：FrameScheduler + Dirty Flags（P0）
**目标**
- 空闲态不渲染；输入/动画/定时器/资源准备完成触发渲染。
- 将帧阶段拆成 `needs_layout/needs_paint/needs_present`，便于局部优化与测量。

**涉及文件**
- `crates/zeno-shell/src/window.rs`：移除 `about_to_wait` 的无条件 redraw；接入 scheduler。
- `crates/zeno-runtime`：新增 `frame_scheduler.rs`（或放在 shell，但建议 runtime 统一策略），集中决定“下一帧是否需要、需要做哪些阶段”。
- `crates/zeno-core/src/config.rs`：增加调试开关（强制连续绘制、帧统计输出等）。

**验收标准**
- 空闲时 CPU 占用显著下降；只有窗口事件/显式 invalidate 才触发 redraw。

### 3) Compose 引入 retained tree + diff + dirty propagation（P1）
**目标**
- 从“每次 compose 全树测量/全量 Scene”演进为“保留式树 + 局部 invalidation + dirty subtree 更新”。

**涉及文件**
- `crates/zeno-compose/src/node.rs`：为节点引入稳定 `NodeId` 与可 diff 的结构。
- `crates/zeno-compose/src/layout.rs`：加入布局缓存与 dirty subtree 计算路径。
- `crates/zeno-compose/src/render.rs`：发射阶段避免大对象 clone，优先复用缓存片段。
- 新增：`tree.rs`、`diff.rs`、`invalidation.rs`（文件名按现有风格落地）。

**验收标准**
- 小改动（仅颜色/位置/文本内容）不会触发全树测量与全量 Scene 重建。

### 4) Scene 模型升级为“批处理友好 + 资源句柄化”（P1/P2）
**目标**
- 保留 `DrawCommand` 的易用性，同时引入更强的结构：资源键、layer/clip/transform 栈、可局部更新的节点块。

**涉及文件**
- `crates/zeno-graphics/src/scene.rs`：从扁平 commands 逐步引入结构化 scene（可分阶段：先加资源键，再加 layer/clip）。
- `crates/zeno-backend-skia/src/real.rs` 与 `crates/zeno-backend-impeller/src/*`：利用资源键做缓存，减少重复构建。

**验收标准**
- 相同文本/相同路径重复绘制时，后端可命中缓存（typeface、glyph atlas、path 等）。

### 5) 文本系统升级为可插拔 shaping + cache（P2）
**目标**
- 将 `FallbackTextSystem` 明确降级为“无平台能力兜底/测试用”，主路径引入真实 shaping 与缓存接口。

**涉及文件**
- `crates/zeno-text/src/system.rs`：拆出 `TextSystem/TextShaper/TextCache` 或等价抽象。
- `crates/zeno-text/src/types.rs`：引入可缓存的 paragraph key、glyph run、line metrics 结构。
- 后端侧：Skia/Impeller 分别接入 glyph cache / paragraph cache。

**验收标准**
- 长文本/多语言场景下的布局与绘制成本可控，且布局结果与渲染一致。

### 6) 开发体验：feature preset、bench gallery、可观测性（贯穿全程）
**目标**
- 新用户 1 条命令跑通“桌面验证”；开发者有基线 benchmark、scene/layout dump、frame stats。

**涉及文件**
- Workspace `Cargo.toml` 与相关 crate `Cargo.toml`：提供平台 preset feature（例如 `desktop_demo` 已存在，可扩展为更细预设）。
- `examples/`：新增 `bench_gallery`（深树布局、长文本、滚动/动画等）。
- `docs/architecture`：增加 `performance-plan.md`/`devtools-plan.md`（如需要）。

**验收标准**
- 每次优化有可重复对比的场景与指标；问题定位不依赖读源码。

## Assumptions & Decisions
- 性能优先：先解决帧调度、retained/diff、缓存与抽象收敛，再扩展更复杂组件体系。
- 真正全平台：抽象优先于单平台堆实现，但桌面仍作为最先验证与压测的主战场。
- 依赖隔离：核心库保持轻量，重依赖（winit/skia-safe/metal 等）持续通过 feature 隔离。

## Verification Steps
- `cargo check --workspace`
- `cargo test --workspace`
- 运行 `examples/minimal_app` 验证：
  - 默认策略下的 backend 解析结果与回退信息（attempts）。
  - 空闲态不持续 redraw（帧调度改造后）。
- 基准验证（引入 bench_gallery 后）：
  - 深树布局、长文本、多次小更新三类场景的帧时间与 CPU 占用对比。

## Deliverables
- 一套对齐当前代码现状的架构路线图（本文）。
- 后续实施时按优先级拆分为可独立合并的改造 PR/任务：RenderSession、FrameScheduler、retained tree、Scene 结构化、Text shaping/cache、Devtools/bench。

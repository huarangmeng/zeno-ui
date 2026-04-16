# DisplayList + Compositor Architecture

## 状态
- 状态：`DisplayList -> DamageRegion -> TilePlan/RasterBatch/CompositePass -> backend` 的最小闭环已经落地，当前进入“把规划/执行职责继续从 session 前推到独立 compositor 层，并补齐 compositor-only 增量语义”的阶段。
- 阶段判断：当前运行时主链路已经完成 `FrontendObjectTable + DirtyTable + LayoutWorkQueue + LayoutArena + RetainedDisplayList + DisplayList + CompositorFrame` 的 V2 对象表架构；`zeno-compositor` 已不再只是 frame/damage 协议壳，而是已经具备 `TileGrid + TileCache + TileResourcePool + CompositorLayerTree + CompositeExecutor + CompositorService` 的最小实现。Skia 与 macOS Impeller 桌面路径都已能真实消费 `RasterBatch + CompositePass`；当前真正的缺口已收敛为职责归属、语义完备度与跨平台收敛，而不是“是否要切到 DisplayList”。

## 目标
- 将渲染链路拆分为 `Paint -> Rasterize -> Composite` 三个解耦阶段。
- 让 `transform / opacity / blend / effect` 类更新优先走 compositor-only 路径，而不是重新 paint。
- 让局部更新成本与脏区域、脏 tile 数量相关，而不是与整棵场景树规模相关。
- 让后端消费统一的 DisplayList/Compositor 协议，而不是每帧重建 layer/object 索引、HashMap 和排序缓存。

## 非目标
- 不保留对旧 `Scene/RenderSceneUpdate` 协议的兼容层。
- 不保留任何 `RetainedScene` 双轨兼容层。
- 不接受“新协议包旧协议”或“旧协议投影新协议”的过渡实现。
- 不讨论上层声明式 API 表面形态。
- 不讨论平台 session 选择与窗口系统策略；这些继续由 `zeno-runtime` 与 `zeno-platform` 负责。

## 当前代码已完成的部分
- 运行时热路径的 retained 过渡层已经完成历史使命，并已被从公开 API 与主提交流中移除。
- `RenderSession`、`Renderer`、desktop/mobile presenter 已切到 `DisplayList` 主提交模型；`submit_scene(RenderSceneUpdate)` 与旧 retained/scene delta 提交类型都已退出代码现状。
- `UiRuntime` / `AppHost` / `RenderSession` 已打通 `submit_compositor_frame(...)` 单轨主提交流程，`DisplayList` 作为 `CompositorFrame` payload 提交。
- `zeno-ui` 已具备 `RetainedDisplayList`、`SpatialTree`、`ClipChainStore`、`StackingContext` 与 `DisplayList` 快照输出。
- Skia 与 macOS Impeller 已具备原生 `DisplayList` renderer，不再依赖任何 retained scene backend。
- `DisplayList` 当前已经可以构建 `CompositorSubmission { TilePlan, RasterBatch, CompositePass, CompositorLayerTree }`，说明 raster/composite 中间协议边界已经存在，不再只是设计目标。
- `zeno-compositor` 已具备 `DamageRegion`、`DamageTracker`、`TileGrid`、`TileCache`、`TileResourcePool`、`CompositorLayerTree`、`CompositeExecutor`、`CompositorService` 与线程化 worker/scheduler 最小闭环。
- Skia 桌面路径已按 tile 真实渲染到 offscreen surface，并基于 `CompositeExecutor` 产出的 layer/tile jobs 执行 composite；macOS Impeller 也已持有真实 tile texture，并消费 layer job 的 blend/effect 参数合成到 drawable。
- 统一资源同步已经接入主链：`TileResourcePool` 负责跨 backend 同步分配、复用、释放与淘汰句柄，桌面双后端不再各自维护一套不透明 tile 生命周期。
- 图片资源已经从“节点内联像素”升级为稳定资源链路：`ImageSource` 会产出可复用的 `ImageResourceKey`，`DisplayImage` 已显式携带 `cache_key`，`DrawCommand::Image` 也复用同一资源键语义。
- 图片缓存主链已在桌面双后端打通：Skia 已基于 `DisplayImage.cache_key` 复用 `sk::Image`，macOS Impeller 已基于同一资源键复用 Metal texture；Impeller 还会在 offscreen stacking context 路径复用 `OffscreenContextCache`，避免每帧/每 tile 重复创建 GPU 资源。
- macOS Impeller 的 `DisplayList` 热路径已增加 render-time lookup table，一次性预建 `spatial / clip chain / stacking context / context bounds` 索引，后续 raster/composite 不再反复线性查找。
- retained 过渡层曾经把 packet arena 回收、slot compact、派生查询缓存这些方向探索得比较深。
- 这意味着“结构化场景提交层”的历史优化空间已经被吃得比较深，继续回到 retained 协议不会再带来正确的长期收益。

## 当前还没有做的部分
- 桌面后端已进入真实 tile raster/composite 路径，但移动端 session 仍主要把 `CompositorSubmission` 作为统计与过渡协议，执行层尚未像桌面一样完全切到统一 tile resource + layer composite 模型。
- 没有 compositor-only 动画：`transform / opacity / blend / effect` 更新仍然会回落到 paint/scene dirty。
- `CompositorService` 虽已线程化，但生命周期仍挂在 session 内部，尚未形成真正独立于 presenter/session 的 compositor 子系统；present 也仍然由 session 驱动。
- backend 无关的 raster/composite 协议骨架已经存在，但 layer 依赖关系、复杂 effect graph、offscreen 资源生命周期与更细粒度复用策略仍是 MVP。
- 图片资源主链已完成比 MVP 更进一步的闭环：`ImageNode -> ImageSource -> ImageResourceKey/ImageResourceTable -> DisplayImage(cache_key)/DrawCommand::Image -> backend` 已打通，且 stable key、backend texture cache、offscreen cache 已进入主链；当前仍缺的是多来源图片（asset/path/url）、解码缓存、失效策略与更完整的自定义资源来源。
- 没有 path 级 clip 与更复杂 filter graph：当前 clip/effect 已可运行，但仍偏 MVP，尚未升级到完整 compositor 级语义。

## 当前架构的结构性瓶颈

### 1. 职责边界已理顺，瓶颈已前移到 graph 语义
- 当前 planning 已收口到 `zeno-compositor::CompositorPlanner`，`zeno-scene` 不再持有 `CompositorSubmission` 构建逻辑。
- 当前 `CompositorLayerTree` 已经具备最小 `parent / child_layers / descendant_layers / paint_order / scope_entries / subtree_bounds` 元数据，`StackingContext` 也已显式携带父 context 关系，不再依赖 backend 通过 `SpatialTree` 反推父链。
- Skia / Impeller 的 tile raster 路径当前已经能直接消费这份 `scope_entries` 顺序元数据，不再各自私有推导“direct item 与 child context 的首次出现顺序”。
- 现在真正未完成的已经不是 planning 职责归属，而是 layer graph 对跨 layer 依赖、复杂 effect graph、以及更强的 scope 级排序语义表达仍然偏简化。
- 这意味着职责边界已经理顺，下一阶段要继续加深的是 graph 语义，而不是再做一轮模块搬迁。

### 2. layer/effect 图仍然是 MVP
- 当前 `CompositorLayerTree` 仍主要由 root layer + stacking contexts 派生；虽然已经有最小父子关系、子树依赖、稳定 paint 顺序与 `scope_entries` 元数据，但跨 layer 依赖、更复杂的 scope 排序语义仍然偏简化。
- `CompositeExecutor` 已能产出 layer jobs，但对复杂 effect graph、跨 layer 依赖、跨 tile offscreen 资源生命周期的表达还不充分。
- 这意味着当前实现已经能支撑最小 blend/effect/offscreen 语义，但还不是最终的 compositor graph executor。

### 3. 跨平台执行路径还没有完全收敛
- 桌面 Skia/Impeller 已真实执行 tile raster/composite，但移动端仍主要停留在“带 submission 统计的 `DisplayList` 提交”阶段。
- 同一套 `CompositorSubmission` 已存在，但还没有在所有 backend/session 上都形成一致的资源池、raster、composite 执行栈。
- 如果这一步不补齐，后续统计、调优与语义能力会继续在桌面/移动之间分叉。

### 4. compositor-only dirty 分类仍未进入主链
- 当前 `transform / opacity / blend / effect` 更新仍然会回落到 paint/layout 相关脏路径。
- 理想路径中，这类更新只应修改 `SpatialTree` / `StackingContext` 或 layer graph 参数，并将 raster 复用到最大化。
- 这仍然是当前架构距离真正“动画不触发 paint”的最大差距。

### 5. clip / effect / image 资源语义仍偏最小实现
- clip 目前主要是 bounds/rounded-bounds，尚未升级到 path 级 clip。
- effect 目前支持 blur/drop-shadow，macOS Impeller 已有 `OffscreenContextCache` 复用离屏结果，但仍没有更完整的 filter graph 与跨 backend 统一资源生命周期策略。
- 图片资源已完成 `Rgba8 + stable key + backend image cache` 闭环，但多来源、解码缓存、失效策略与更系统化的资源统计仍未完成。

## 终极架构总览

### 总体分层
- `zeno-ui`：维护 `FrontendObjectTable`、`DirtyTable`、`LayoutWorkQueue`、`LayoutArena`，并负责产出 `DisplayList`。
- `zeno-scene`：不再承担“场景树渲染协议”角色，直接转为 `DisplayList + SpatialTree + ClipChain + StackingContext` 协议层。
- `zeno-compositor`：新增 compositor 层，负责 `DamageTracker`、`TileGrid`、`TileCache`、`CompositorLayerTree` 与合成线程。
- `zeno-backend-*`：长期目标是不再直接消费任何 scene/retained 中间协议，而是消费 raster/composite 协议。

### 三阶段渲染模型
1. Paint（UI 线程）
- 输入：`FrontendObjectTable + LayoutArena + DirtyTable`
- 输出：`DisplayList`
- 职责：只描述“画什么”

2. Rasterize（Compositor 线程，可并行）
- 输入：`DisplayList + DamageRegion`
- 输出：更新后的 `TileCache`
- 职责：只处理“哪些 tile 需要重栅格化”

3. Composite + Present（Compositor 线程）
- 输入：`CompositorLayerTree + TileCache + CompositePass`
- 输出：最终 surface
- 职责：只处理“按 layer 顺序把 tiles 合成到屏幕”

## 核心协议

### DisplayList

```rust
pub struct DisplayList {
    pub viewport: Size,
    pub items: Vec<DisplayItem>,
    pub spatial_tree: SpatialTree,
    pub clip_chains: ClipChainStore,
    pub stacking_contexts: Vec<StackingContext>,
    pub generation: u64,
}
```

DisplayList 是 paint 阶段的唯一输出，表达当前帧的绘制意图，不直接承担 GPU 合成职责。

### DisplayItem

```rust
pub struct DisplayItem {
    pub item_id: DisplayItemId,
    pub spatial_id: SpatialNodeId,
    pub clip_chain_id: ClipChainId,
    pub stacking_context: Option<StackingContextId>,
    pub visual_rect: Rect,
    pub payload: DisplayItemPayload,
}
```

- `visual_rect`：世界空间下的可见包围盒，用于 damage 与 tile 分配。
- `spatial_id`：引用变换树。
- `clip_chain_id`：引用裁剪链。
- `stacking_context`：仅在需要特殊合成语义时存在。

建议的 payload 方向：
- `FillRect`
- `FillRoundedRect`
- `TextRun(DisplayTextRun)`
- `Image(DisplayImage)`
- `Custom`

当前代码现状：
- `TextRun` 已直接携带 `position + TextLayout + color`
- `Image` 已直接携带 `cache_key + dest_rect + width + height + rgba8`
- 图片来源已不再直接依赖节点内联裸像素：`ImageNode` 当前通过 `ImageSource` 引用资源，builder 在生成 `DisplayList` 时经由 `ImageResourceTable` 解析为 `DisplayImage`
- `ImageSource` 当前会缓存 `ImageResourceKey` 的计算结果，避免在 compose / reconcile / display-list build 过程中重复全量遍历 RGBA 像素
- 这意味着文本与图像 payload 已经是“可直接渲染且带稳定资源身份”的数据面，而不是只持有 cache key 的占位协议

### SpatialTree

```rust
pub struct SpatialTree {
    pub nodes: Vec<SpatialNode>,
}

pub struct SpatialNode {
    pub id: SpatialNodeId,
    pub parent: Option<SpatialNodeId>,
    pub local_transform: Transform2D,
    pub world_transform: Transform2D,
    pub dirty: bool,
}
```

- `transform` 必须从绘制 payload 中剥离，作为独立真相源。
- `world_transform` 由父子链缓存，只有该节点或祖先 dirty 时重算。
- 终局目标：transform 动画只修改 `SpatialNode.local_transform`，不触发 repaint。

### ClipChainStore

```rust
pub struct ClipChainStore {
    pub chains: Vec<ClipChain>,
}

pub struct ClipChain {
    pub id: ClipChainId,
    pub spatial_id: SpatialNodeId,
    pub clip: ClipRegion,
    pub parent: Option<ClipChainId>,
}
```

- 裁剪链支持复用与共享，不再复制到每个渲染对象。
- 目标语义：clip 变化优先转换为 damage，而不是立即触发 paint。

### StackingContext

```rust
pub struct StackingContext {
    pub id: StackingContextId,
    pub spatial_id: SpatialNodeId,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub effects: Vec<Effect>,
    pub needs_offscreen: bool,
}
```

- 对应当前 layer/effect/offscreen 的合成语义，但不再直接等价于场景快照 layer。
- `opacity < 1.0`、非 Normal blend、blur、drop shadow 等进入 stacking context。
- 目标语义：`opacity` 与部分 effect 参数更新优先在 compositor 执行。

## 保留式数据面：RetainedDisplayList

```rust
pub struct RetainedDisplayList {
    pub items: Vec<DisplayItem>,
    pub spatial_tree: SpatialTree,
    pub clip_chains: ClipChainStore,
    pub stacking_contexts: Vec<StackingContext>,
    pub object_item_ranges: Vec<Option<Range<usize>>>,
    pub free_item_slots: Vec<usize>,
    pub generation: u64,
}
```

作用：
- 替代旧时代的 `FragmentStore + RetainedScene + RenderObjectDelta` 组合。
- 让脏对象只更新自己对应的 item 区间，而不是重新收集整棵 patch。
- 为 compositor 提供稳定、可增量快照化的 DisplayList 来源。

更新规则：
- paint dirty：重建对象对应的 `DisplayItem` 区间。
- layout dirty：更新 `visual_rect` 与受影响的 `spatial/clip` 引用。
- 目标规则：`transform/opacity/blend/effect` dirty 优先修改 `SpatialTree/StackingContext`，不重建 item。

## Compositor 层

### DamageRegion

```rust
pub enum DamageRegion {
    Empty,
    Rects(Vec<Rect>),
    Full,
}
```

damage 生成规则：
- item 变更：`old.visual_rect union new.visual_rect`
- spatial 变更：受该 spatial 节点影响的所有 item 旧/新世界边界并集
- stacking context 变更：该 context 覆盖范围

原则：
- 允许保守扩大 damage。
- 禁止漏报任何可能变化的像素区域。

### TileGrid + TileCache
- viewport 按固定 tile 大小切分，建议从 `256x256` 起步。
- 每个 tile 记录：
  - 自身边界
  - 覆盖它的 `DisplayItemId` 列表
  - rasterized 结果句柄
  - dirty 标记
- damage 只会使相交 tile 失效，而不是整帧全量重栅格化。

### CompositorLayerTree
- 由 `StackingContext` 派生，而不是直接复用 paint 侧结构。
- 管理 compositor 侧的离屏合成、layer 顺序、tile 归属与 present。
- 在 transform/opacity 动画中，CompositorLayerTree 可以独立推进而不依赖 UI 线程 repaint。

## 帧流水线

### 首帧
1. `FrontendObjectTable` 编译完成。
2. `LayoutWorkQueue` 产出 `LayoutArena`。
3. Paint 阶段全量生成 `RetainedDisplayList`。
4. 快照化为 `DisplayList` 并提交给 backend 原生 renderer。
5. 当前阶段由 backend 直接执行 raster + composite；终局方案再演进为独立 compositor 建立 tile grid、分配 items 到 tiles。
6. composite + present。

### Paint-only 增量帧
1. reconcile 标出 paint dirty 对象。
2. `RetainedDisplayList` 仅替换脏对象的 item 区间。
3. 基于脏对象旧/新子树 bounds 计算 `DamageRegion`。
4. compositor 仅重栅格化脏 tiles。
5. composite + present。

### Layout 增量帧
1. 运行局部 relayout。
2. 更新受影响 item 的 `visual_rect`。
3. 更新相关 spatial/clip 引用。
4. 基于 layout dirty roots 的旧/新子树 bounds 计算 geometry `DamageRegion`。
5. compositor 仅处理受影响 tiles。

### Transform / Opacity 动画帧
1. 目标态只更新 `SpatialTree` 或 `StackingContext`。
2. 不重建 DisplayItem，不重做 paint。
3. compositor 依据新的 transform/opacity 重新 composite。
4. 若 tile 内容未变，则无需重新 rasterize。

这是方案 C 相比当前架构最大的质变：动画不再强耦合 UI paint。

## 与当前协议的映射关系

### 可直接继承的部分
- `FrontendObjectTable` 继续作为运行时对象表唯一真相源。
- `DirtyTable` 继续作为脏标记入口，但要区分 paint-dirty 与 compositor-dirty。
- `LayoutWorkQueue + LayoutArena` 继续负责布局。
- `Modifier -> Style` 依旧是唯一声明式语义入口。

### 需要废弃或重写的部分
- `FragmentStore`：被 `RetainedDisplayList` 替代。
- `RetainedScene`：已退出 runtime/backend 主渲染协议，并完成了“消除快照合并成本”的历史使命。
- `Scene`：仅保留为测试/调试快照结构，退出主热路径。
- `RenderObject / LayerObject`：被 `DisplayItem / StackingContext / CompositorLayer` 分解。
- `RenderObjectDelta / RenderSceneUpdate`：被 `DisplayList + DamageRegion + Compositor updates` 替代。
- `apply_delta_in_place()`：从主渲染协议退场。
- `collect_scene_patch_items()`：彻底删除。

## 还没有做的关键模块

### 1. `DisplayList` 协议层
- 状态：已完成
- 已有 `DisplayItemId / SpatialNodeId / ClipChainId / StackingContextId`。
- 已有 `SpatialTree`、`ClipChainStore`、`StackingContext` 作为独立真相源。

### 2. `RetainedDisplayList`
- 状态：已完成
- 已有 `object_item_ranges`、`free_item_slots`、item 区间更新能力。
- 已能表达“paint dirty 只替换 item range，layout dirty 只改 geometry/spatial/clip”的数据面。

### 2.5 图片资源数据面
- 状态：已完成主链 MVP，并补齐稳定 key 与 backend 资源缓存
- 已有 `ImageSource::Rgba8`、`ImageResourceKey`、`ImageResourceTable` 与 `ImageNode -> resource table -> DisplayImage` 解析链路。
- `DisplayImage` 已显式携带 `cache_key`，`DrawCommand::Image` 也会导出对应 `SceneResourceKey`，后端可围绕统一图片身份做缓存。
- `ImageSource` 当前已缓存 `ImageResourceKey` 的计算结果，避免同一块 `Arc<[u8]>` 在多次 compose/reconcile 中重复全量 hash。
- 桌面双后端当前都已基于 `cache_key` 复用图片资源：Skia 复用 `sk::Image`，macOS Impeller 复用 Metal texture，避免同图在多 tile / 多帧场景下重复重复构建后端图片对象。
- 待补 `ImageSource::Asset/Path/Url`、解码缓存、失效策略与 backend 无关的资源生命周期统计。

### 3. `zeno-compositor`
- 状态：已完成最小协议层，并具备 tile 规划、统一资源池、layer/effect tree、composite executor 与 raster/composite 中间协议骨架
- crate 结构已按职责拆分，不再把所有实现堆在 `src/lib.rs`：当前主模块已收敛为 `damage.rs / frame.rs / tile.rs / composite.rs / scheduler.rs / tests.rs`，`lib.rs` 仅保留模块声明与 re-export。
- 已有 `DamageRegion`、`DamageTracker`、`TileGrid`、`TileCache`、`TileResourcePool`、`TilePlan`、`CompositorLayerTree`、`RasterBatch`、`CompositePass`、`CompositeExecutor`、`CompositorService`、`CompositorSubmission`、`CompositorFrame` 与过渡期 `CompositorFrameStats`。
- 当前 `TileGrid` 已能把 `DamageRegion` 映射为稳定的脏 tile 集合，`TileCache` 已能基于前一帧缓存状态生成 `reraster/reused` 规划并分配 `TileContentHandle`，同时维护最小 `TileContentSlot + TileResourceDescriptor` 状态，并支持句柄复用、基础老化淘汰、字节预算约束与释放旧句柄；`TileResourcePool` 已负责跨 backend 同步活跃/释放/复用资源句柄。
- `CompositorLayerTree` 已能从 `DisplayList.stacking_contexts` 与 item 覆盖范围派生 layer 树，并保留 `parent / child_layers / descendant_layers / paint_order / scope_entries / subtree_bounds / blend_mode / effects / effect_bounds / effect_padding` 元数据；`CompositePass` 也已升级为 layer-aware composite steps。
- `CompositeExecutor` 不再只输出 tile 级计划：当前会同时产出 `layer_jobs + tile_jobs` 两层执行计划，layer job 已携带 `blend_mode`、`effects`、`bounds`、`effect_bounds`、`needs_offscreen` 等信息，供 backend 在 composite 阶段消费。
- 帧级统计当前已覆盖 `dirty_tile_count`、`cached_tile_count`、`reraster_tile_count`、`raster_batch_tile_count`、`composite_tile_count`、`compositor_layer_count`、`offscreen_layer_count`、`tile_content_handle_count`、`compositor_task_count`、`compositor_queue_depth`、`compositor_dropped_frame_count`、`compositor_processed_frame_count`、`released_tile_resource_count`、`evicted_tile_resource_count`、`budget_evicted_tile_resource_count`、`age_evicted_tile_resource_count`、`descriptor_limit_evicted_tile_resource_count`、`reused_tile_resource_count`、`reusable_tile_resource_count`、`reusable_tile_resource_bytes`、`tile_resource_reuse_budget_bytes`、`compositor_worker_threaded`、`compositor_worker_alive`、`composite_executed_layer_count`、`composite_executed_tile_count`、`composite_offscreen_step_count`。
- 已有线程驱动的 `CompositorService` / `ThreadedCompositorWorker` / `CompositorScheduler` loop 并接入 desktop session；`CompositeExecutor` 也已真实产出 backend 可执行的 layer/tile composite jobs；`CompositorPlanner` 已统一承接 `CompositorSubmission` planning。当前仍缺更完整的 layer 依赖关系、跨 backend 更精细的 GPU 资源复用/淘汰策略，以及移动端执行路径收敛。

### 4. backend 新输入协议
- 状态：已完成第一阶段，并接入最小 compositor frame 与 raster/composite 中间协议边界
- Skia 与 macOS Impeller 已能原生消费 `DisplayList`，平台 `RenderSession` 已切到 `CompositorFrame` 提交；当前 session 已先把 frame 规划为 `CompositorSubmission { TilePlan, RasterBatch, CompositePass, CompositorLayerTree }`，并基于 `TileGrid + TileCache` 统计 `dirty_tile_count`、`cached_tile_count`、`reraster_tile_count`、`raster_batch_tile_count`、`composite_tile_count`、`compositor_layer_count`、`offscreen_layer_count`。
- 执行层状态：Skia 桌面路径已按 `RasterBatch` 逐 tile 渲染到真实 offscreen tile surface，并在 composite 阶段真实消费 layer job 的 `blend_mode/effects/effect_bounds`；macOS Impeller 也已持有真实 tile texture 资源，并在 composite 阶段消费 layer job 的 blend/effect 参数，把 tile texture 合成到 drawable。移动端当前仍主要复用 `DisplayList` presenter 渲染能力，尚未完全对齐桌面执行栈。

## 直接实现方案（从当前状态继续推进）

### 原则
- 不再新增任何新的 scene/retained 桥接层。
- 新增渲染能力优先接入 `DisplayList` 主链，而不是回到任何 retained 协议。
- retained 兼容链已从公开 API 与主渲染链移除；后续不再保留任何 retained 提交边界。
- 后续改造以增量收敛为主，而不是重新设计一条“切换前/切换后”双文档路线。

### 直接切换顺序

#### Step 1：在 `zeno-scene` 直接落地新协议
- 新增：
  - `DisplayList`
  - `DisplayItem`
  - `SpatialTree`
  - `ClipChainStore`
  - `StackingContext`
  - `RetainedDisplayList`
- 同一步里约束：
  - retained scene 不再作为公开协议存在
  - 新增代码不得依赖任何 retained 提交类型

当前结果：
- 已完成；`DisplayList`、`RetainedDisplayList`、`DisplayTextRun`、`DisplayImage` 已在 `zeno-scene` 落地并进入主链。

#### Step 2：在 `zeno-ui` 直接替换 `FragmentStore + collect_scene_patch_items`
- 删除 `FragmentStore` append-only 模型与 patch collect 机制。
- `ComposeRenderer` 不再产出 `RenderObject/LayerObject` patch，而是直接写入 `RetainedDisplayList`：
  - paint dirty：替换对象对应的 item 区间
  - layout dirty：只更新 `visual_rect` 与 `spatial/clip` 引用
  - transform/opacity/blend/effect dirty：只更新 `SpatialTree/StackingContext`，禁止重建 item payload

当前结果：
- 已完成；`zeno-ui` 已直接写入 `RetainedDisplayList` 并快照为 `DisplayList`。

#### Step 3：新增 `zeno-compositor` crate，并让 runtime 直接提交 compositor 输入
- 引入：
  - `DamageRegion` 与 damage 计算
  - `TileGrid/TileCache`
  - `CompositorLayerTree`
  - compositor thread 与调度
- `UiRuntime/AppHost` 输出从“对象树更新”切到：
  - `DisplayList delta`
  - `DamageRegion`
  - `Compositor updates`

当前结果：
- 部分完成；`zeno-compositor` crate、`DamageRegion`、`DamageTracker`、`TileGrid`、`TileCache`、`CompositorLayerTree`、`CompositorFrame` 已落地，`UiRuntime/AppHost/RenderSession` 已直接提交 compositor frame。
- 当前仍未完成的是更完整的 layer 依赖/排序模型、跨 backend 更细粒度的 GPU 资源复用策略与真正独立于 session 生命周期的 compositor 进程/服务；damage 仍是保守区域语义，但已经具备统一资源池与 backend 实际资源落地。

#### Step 4：平台层直接切换到 compositor frame
- 收敛 `RenderSession` / `Renderer` 到 compositor frame 提交模型：
  - `CompositorFrame`（或等价的 `RasterBatch + CompositePass`）
  - backend 不再直接遍历 `DisplayList`

当前结果：
- 已完成第一阶段；当前 `RenderSession` 已收口为 `submit_compositor_frame(...)` 单轨，desktop/mobile session 会先把 frame 规划为 `CompositorSubmission`，再映射到现有 full/region render 路径。
- 下一步不只是把 backend 内部继续上推到真正消费 `RasterBatch + CompositePass` 的执行路径，还要让 planning/execution 生命周期不再深绑在 session 内部。

#### Step 5：backend 直接改成 raster/composite 消费者
- Skia：tile rasterizer + compositor canvas backend
- Impeller：tile texture producer + compositor pass executor
- backend 不再知道 `RenderObject / LayerObject / DrawOp`

当前结果：
- 已完成前置协议阶段；Skia 与 macOS Impeller 都已具备原生 `DisplayList` renderer，且 session 侧已经具备 backend 无关的最小 `RasterBatch + CompositePass + CompositorLayerTree` 协议骨架。
- 当前已完成的执行侧收敛是：Skia 与 macOS Impeller 桌面路径都会真实消费 `RasterBatch` 做 tile 级局部执行，并分别持有真实 offscreen surface / tile texture 资源；session 侧也已通过 `CompositorService` 调度 `Raster -> Composite -> Present` 任务序列，且 `CompositeExecutor` 已真实产出并驱动 backend 可执行的 tile composite jobs。
- macOS Impeller 当前还进一步补齐了执行期缓存与索引：图片 texture cache、offscreen context cache，以及 render-time lookup table 已进入主渲染路径，用于压低 tile raster/composite 的 CPU 与 GPU 重复开销。
- 仍未完成的是移动端执行路径对齐、跨 backend 更完整的 GPU 资源复用/淘汰策略、backend 直接基于 layer/effect graph 执行更复杂的 `CompositePass`、更完整的 layer executor，以及独立 compositor 层。

## 与 Modifier 的语义映射
- `transform / transform_origin` -> `SpatialTree`
- `clip` -> `ClipChainStore`
- `opacity / blend_mode / blur / drop_shadow / layer` -> `StackingContext`
- `background / corner_radius / text/image payload` -> `DisplayItemPayload`
- `TextStyle.color / font_size / font_family / font_weight / italic / font_feature(s) / letter_spacing / line_height / text_align` -> `Style.text -> TextParagraph / DisplayTextRun`

这保持了 `Modifier -> Style -> Render Protocol` 的单向依赖，不引入第二语义真相源。

## 分阶段落地顺序

### 已完成
- `DisplayList`-only renderer/session/presenter 边界
- retained query/cache/arena 优化
- `DisplayList`
- `RetainedDisplayList`
- Skia 原生 `DisplayList` renderer
- macOS Impeller 原生 `DisplayList` renderer

### 下一阶段必须直接完成
- 移动端对齐桌面 `TileResourcePool + RasterBatch + CompositePass` 执行栈
- 更完整的 `CompositorLayerTree` 依赖/排序模型与 layer executor
- compositor-only 动画路径（transform/opacity/blend/effect）
- image/resource/cache 正式来源链路
- path clip 与更完整 effect/filter graph

### 当前可保留但不再扩张的部分
- retained query/cache/arena 优化
- 任何把 scene/retained 语义重新暴露回 runtime/platform/backend 边界的新抽象

## 关键验收标准
- paint-only 更新不再执行全树 patch collect。
- `transform / opacity` 动画帧中，UI 线程 paint 次数必须为 0 或严格受控。
- 局部更新成本与脏 tile 数相关，而不是与对象总量相关。
- backend 不再自行承担完整的 damage/tile/composite 规划职责，而是消费统一 compositor 协议。
- `zeno-scene` 保持协议/数据面角色，`CompositorSubmission` 的 planning 与执行生命周期主要由 compositor 层承担。
- DisplayList、DamageRegion、TileCache、CompositorLayerTree 都具备稳定的统计与调试出口。

## 风险与约束
- tile 过小会增大管理开销，过大会降低局部更新收益。
- clip/effect 与 tile 边界叠加时需要严格保证不漏画。
- 跨 tile 的 offscreen/effect 合成需要单独设计资源生命周期。
- 文本对象最终应进一步句柄化，配合未来 `TextObjectTable` 共享 shaping/layout/glyph cache。

## 结论
- 当前系统已经把 retained 过渡层这条路走到比较深的位置，但它的收益边界已经清楚暴露出来了。
- 真正还没有做的，不是继续优化 retained 过渡层，也不是继续证明 `DisplayList` 可行，而是把已经落地的 `DisplayList` 主链继续升级为 `Damage + Tile + Compositor`。
- 后续实现应避免新增任何 scene/retained 桥接层，把所有新增能力优先落在 `DisplayList -> compositor` 方向上。

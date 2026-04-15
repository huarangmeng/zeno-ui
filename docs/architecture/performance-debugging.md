# Performance Debugging

## 目标

- 保留一组稳定的性能观测点，用于后续定位 `prepare/layout`、`submit/raster`、`offscreen context`、`text layout` 等热点。
- 这些日志点属于长期维护的性能观测面，不是一次性调试残留；如需改名或删除，必须同步更新本文件。
- 推荐默认使用 `trace` 级别按需打开，避免污染常规运行日志。

## 推荐命令

```bash
ZENO_DEMO_BACKEND=impeller \
ZENO_DEMO_FRAME_STATS=1 \
RUST_LOG=zeno.session=trace,zeno.runtime=info \
cargo run -p minimal_app
```

按主题筛选：

```bash
grep -E "app_host_frame_timing|compose_update_branch|text_layout_node|impeller_submit_timing|impeller_offscreen_context|impeller_offscreen_context_timing|impeller_composite_drawable"
```

## 观测层次

### 1. 应用帧级

- `op = "app_host_frame_timing"`
- 位置：`crates/zeno-runtime/src/host/app_host.rs`
- 用途：看单帧的 `prepare_ms / submit_ms / total_ms`，先判断瓶颈是在 UI 侧还是 backend 侧。
- 关键字段：`prepare_ms`、`submit_ms`、`damage_rect_count`、`dirty_tile_count`、`offscreen_layer_count`

### 2. Compose / Layout 级

- `op = "compose_update_branch"`
- 位置：`crates/zeno-ui/src/render/mod.rs`
- 用途：判断当前帧落在 `full / layout / paint` 哪条分支，并看 dirty roots 与 patch 规模。
- 关键字段：`branch`、`branch_ms`、`dirty_root_count`、`dirty_roots`、`damage_rect_count`

### 3. 单文本布局级

- `op = "text_layout_node"`
- 位置：`crates/zeno-ui/src/layout/work_queue_engine.rs`
- 用途：定位单个文本节点的 shaping / layout 开销。
- 关键字段：`element_id`、`font_size`、`max_width`、`text_len`、`text_layout_ms`
- 说明：当前阈值为 `text_layout_ms > 1.0` 才打印，避免高频噪音。

### 4. Impeller Submit 总览

- `op = "submit_compositor_frame"`
- 位置：`crates/zeno-platform/src/desktop_session/impeller_metal.rs`
- 用途：看一次 compositor submit 的资源、tile、queue、worker 摘要。
- 关键字段：`dirty_tiles`、`cached_tiles`、`raster_tiles`、`composite_tiles`、`offscreen_layers`

- `op = "impeller_submit_timing"`
- 位置：`crates/zeno-platform/src/desktop_session/impeller_metal.rs`
- 用途：看 submit 各阶段耗时拆分，是定位 backend 性能问题的主入口。
- 关键字段：`total_submit_ms`、`worker_ms`、`raster_ms`、`build_tiles_ms`、`drawable_submit_ms`
- 关键缓存字段：`offscreen_cache_entries`、`offscreen_cache_hits`、`offscreen_cache_misses`

### 5. DisplayList Root / Composite 级

- `op = "impeller_display_list_encoder_root"`
- 位置：`crates/zeno-backend-impeller/src/macos_metal/display_list_renderer.rs`
- 用途：看 root encoder 的 dirty region 数与场景规模。
- 关键字段：`preserve_contents`、`dirty_region_count`、`items`、`contexts`

- `op = "impeller_composite_drawable"`
- 位置：`crates/zeno-backend-impeller/src/macos_metal/display_list_renderer.rs`
- 用途：看最终 drawable composite pass 的编码成本。
- 关键字段：`tile_count`、`tile_draw_ms`、`tile_buffer_alloc_ms`、`tile_encode_ms`、`present_commit_ms`

### 6. Offscreen Context 级

- `op = "impeller_offscreen_context"`
- 位置：`crates/zeno-backend-impeller/src/macos_metal/display_list_renderer/context.rs`
- 用途：看某个 offscreen context 的场景范围、effect bounds、patch 请求范围与纹理申请尺寸。
- 关键字段：`context_id`、`effect_bounds`、`raster_scene_rect`、`visible_scene_rect`、`requested_texture_scene_bounds`

- `op = "impeller_offscreen_context_timing"`
- 位置：`crates/zeno-backend-impeller/src/macos_metal/display_list_renderer/context.rs`
- 用途：看单个 offscreen context 的真实成本，以及 cache/grow 路径是否工作正常。
- 关键字段：`texture_alloc_ms`、`texture_grow_copy_ms`、`offscreen_scope_ms`、`composite_back_ms`
- 关键缓存字段：`cache_hit`、`cached_texture_scene_bounds`、`covered_rect`

## 推荐排查顺序

1. 先看 `app_host_frame_timing`，判断 `prepare` 还是 `submit` 更重。
2. 若是 `prepare`，继续看 `compose_update_branch` 与 `text_layout_node`。
3. 若是 `submit`，先看 `impeller_submit_timing`。
4. 若 `raster_ms` 偏高，再看 `impeller_offscreen_context_timing`。
5. 若怀疑最终贴回或 drawable pass，再看 `impeller_composite_drawable`。

## 维护约定

- 保持 `op` 名称稳定；重命名时必须同步更新本文件。
- 新增性能日志时，优先补充到本文件，再决定是否需要长期保留。
- 删除性能日志前，先确认是否已有更高层或更稳定的替代观测点。

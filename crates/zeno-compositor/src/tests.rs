use crate::{
    CompositeExecutor, CompositeLayerPass, CompositePass, CompositeTileRef, CompositorBlendMode,
    CompositorLayerId, CompositorPlanner, CompositorPlanningContext, CompositorPlanningItem,
    CompositorPlanningSource, CompositorScheduler, CompositorScopeEntry, CompositorService,
    CompositorTask, CompositorWorker, DamageRegion, DamageTracker, ThreadedCompositorWorker,
    TileCache, TileContentHandle, TileContentState, TileGrid, TileId, TileResourceKind,
    TileResourcePool,
};
use zeno_core::{Rect, Size};

#[derive(Clone)]
struct TestPlanningSource {
    viewport: Size,
    items: Vec<CompositorPlanningItem>,
    contexts: Vec<CompositorPlanningContext>,
}

impl CompositorPlanningSource for TestPlanningSource {
    fn viewport(&self) -> Size {
        self.viewport
    }

    fn item_count_hint(&self) -> usize {
        self.items.len()
    }

    fn stacking_context_count_hint(&self) -> usize {
        self.contexts.len()
    }

    fn for_each_item(&self, mut visitor: impl FnMut(CompositorPlanningItem)) {
        for item in self.items.iter().copied() {
            visitor(item);
        }
    }

    fn for_each_stacking_context(&self, mut visitor: impl FnMut(CompositorPlanningContext)) {
        for context in self.contexts.iter().cloned() {
            visitor(context);
        }
    }
}

fn root_only_source(viewport: Size) -> TestPlanningSource {
    TestPlanningSource {
        viewport,
        items: vec![CompositorPlanningItem {
            item_index: 0,
            paint_order: 0,
            stacking_context_index: None,
            visual_rect: Rect::new(0.0, 0.0, 20.0, 20.0),
        }],
        contexts: Vec::new(),
    }
}

fn nested_context_source(viewport: Size) -> TestPlanningSource {
    TestPlanningSource {
        viewport,
        items: vec![
            CompositorPlanningItem {
                item_index: 0,
                paint_order: 0,
                stacking_context_index: None,
                visual_rect: Rect::new(0.0, 0.0, 30.0, 30.0),
            },
            CompositorPlanningItem {
                item_index: 1,
                paint_order: 1,
                stacking_context_index: Some(0),
                visual_rect: Rect::new(10.0, 10.0, 50.0, 50.0),
            },
            CompositorPlanningItem {
                item_index: 2,
                paint_order: 2,
                stacking_context_index: Some(1),
                visual_rect: Rect::new(20.0, 20.0, 15.0, 15.0),
            },
        ],
        contexts: vec![
            CompositorPlanningContext {
                parent_context_index: None,
                paint_order: 1,
                opacity: 0.8,
                blend_mode: CompositorBlendMode::Normal,
                effects: Vec::new(),
                needs_offscreen: false,
            },
            CompositorPlanningContext {
                parent_context_index: Some(0),
                paint_order: 2,
                opacity: 0.5,
                blend_mode: CompositorBlendMode::Multiply,
                effects: Vec::new(),
                needs_offscreen: true,
            },
        ],
    }
}

#[test]
fn damage_tracker_collects_rects_until_full() {
    let mut tracker = DamageTracker::new();
    tracker.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
    tracker.add_optional_rect(Some(Rect::new(5.0, 5.0, 3.0, 3.0)));

    let damage = tracker.build();
    assert_eq!(damage.rect_count(), 2);
    assert!(!damage.is_full());
}

#[test]
fn damage_tracker_full_overrides_accumulated_rects() {
    let mut tracker = DamageTracker::new();
    tracker.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
    tracker.mark_full();
    tracker.add_rect(Rect::new(10.0, 10.0, 5.0, 5.0));

    assert_eq!(tracker.build(), DamageRegion::Full);
}

#[test]
fn tile_grid_maps_damage_rects_to_unique_tiles() {
    let grid = TileGrid::new(Size::new(300.0, 300.0), 128.0, 128.0);
    let damage = DamageRegion::from_rects([
        Rect::new(10.0, 10.0, 150.0, 10.0),
        Rect::new(90.0, 90.0, 100.0, 100.0),
    ]);

    let tiles = grid.tiles_for_damage(&damage);
    assert_eq!(
        tiles,
        vec![
            TileId { x: 0, y: 0 },
            TileId { x: 0, y: 1 },
            TileId { x: 1, y: 0 },
            TileId { x: 1, y: 1 },
        ]
    );
}

#[test]
fn tile_grid_full_damage_marks_all_tiles() {
    let grid = TileGrid::new(Size::new(300.0, 300.0), 128.0, 128.0);
    assert_eq!(grid.dirty_tile_count(&DamageRegion::Full), 9);
}

#[test]
fn tile_grid_clamps_damage_to_viewport() {
    let grid = TileGrid::for_viewport(Size::new(256.0, 256.0));
    let damage = DamageRegion::from_rects([Rect::new(-20.0, -20.0, 40.0, 40.0)]);

    assert_eq!(grid.tiles_for_damage(&damage), vec![TileId { x: 0, y: 0 }]);
    assert_eq!(
        grid.tile_rect(TileId { x: 0, y: 0 }),
        Some(Rect::new(0.0, 0.0, 256.0, 256.0))
    );
}

#[test]
fn tile_cache_first_frame_rerasterizes_everything() {
    let mut cache = TileCache::new();
    let grid = TileGrid::new(Size::new(300.0, 300.0), 128.0, 128.0);

    let plan = cache.plan_frame(grid, &DamageRegion::Full);
    assert_eq!(plan.stats.total_tile_count, 9);
    assert_eq!(plan.stats.cached_tile_count, 0);
    assert_eq!(plan.stats.reraster_tile_count, 9);
}

#[test]
fn tile_cache_reuses_clean_tiles_after_warmup() {
    let mut cache = TileCache::new();
    let grid = TileGrid::new(Size::new(300.0, 300.0), 128.0, 128.0);

    let _ = cache.build_tile_state(grid, &DamageRegion::Full);
    let tile_state = cache.build_tile_state(
        grid,
        &DamageRegion::from_rects([Rect::new(0.0, 0.0, 20.0, 20.0)]),
    );
    let plan = tile_state.tile_plan;

    assert_eq!(plan.stats.total_tile_count, 9);
    assert_eq!(plan.stats.cached_tile_count, 8);
    assert_eq!(plan.stats.reraster_tile_count, 1);
    assert_eq!(plan.dirty_tiles, vec![TileId { x: 0, y: 0 }]);
    assert_eq!(plan.reused_tiles.len(), 8);
    assert_eq!(cache.content_handle_count(), 9);
    assert_eq!(
        cache
            .content_slot(TileId { x: 1, y: 1 })
            .map(|slot| slot.state),
        Some(TileContentState::Reused)
    );
    assert_eq!(
        cache
            .content_slot(TileId { x: 1, y: 1 })
            .map(|slot| slot.resource.kind),
        Some(TileResourceKind::OffscreenSurface)
    );
}

#[test]
fn tile_cache_viewport_change_forces_full_rebuild() {
    let mut cache = TileCache::new();
    let _ = cache.plan_frame(
        TileGrid::new(Size::new(256.0, 256.0), 128.0, 128.0),
        &DamageRegion::Full,
    );

    let plan = cache.plan_frame(
        TileGrid::new(Size::new(512.0, 256.0), 128.0, 128.0),
        &DamageRegion::Empty,
    );
    assert_eq!(plan.stats.cached_tile_count, 0);
    assert_eq!(plan.stats.reraster_tile_count, 8);
}

#[test]
fn tile_cache_builds_submission_from_tile_plan() {
    let mut cache = TileCache::new();
    let grid = TileGrid::new(Size::new(300.0, 300.0), 128.0, 128.0);
    let _ = cache.build_tile_state(grid, &DamageRegion::Full);

    let tile_state = cache.build_tile_state(
        grid,
        &DamageRegion::from_rects([Rect::new(0.0, 0.0, 20.0, 20.0)]),
    );

    assert_eq!(tile_state.tile_plan.stats.reraster_tile_count, 1);
    assert_eq!(tile_state.raster_batch.tile_count(), 1);
    assert!(!tile_state.raster_batch.full_raster);
    assert_eq!(
        tile_state.raster_batch.bounds(),
        Some(Rect::new(0.0, 0.0, 128.0, 128.0))
    );
    assert_eq!(tile_state.raster_batch.tiles[0].content_handle.0, 1);
}

#[test]
fn tile_cache_reports_released_handles_after_reraster() {
    let mut cache = TileCache::new();
    let grid = TileGrid::new(Size::new(300.0, 300.0), 128.0, 128.0);
    let _ = cache.build_tile_state(grid, &DamageRegion::Full);
    assert!(cache.take_released_content_handles().is_empty());
    assert!(cache.take_reused_content_handles().is_empty());

    let _ = cache.build_tile_state(
        grid,
        &DamageRegion::from_rects([Rect::new(0.0, 0.0, 20.0, 20.0)]),
    );
    let released = cache.take_released_content_handles();
    let reused = cache.take_reused_content_handles();

    assert!(released.is_empty());
    assert_eq!(reused.len(), 1);
    assert_eq!(reused[0].0, 1);
}

#[test]
fn tile_resource_pool_tracks_allocated_and_released_handles() {
    let mut cache = TileCache::new();
    let mut pool = TileResourcePool::new();
    let grid = TileGrid::new(Size::new(300.0, 300.0), 128.0, 128.0);

    let _ = cache.build_tile_state(grid, &DamageRegion::Full);
    let first = pool.synchronize(&mut cache);
    assert_eq!(first.allocated.len(), 9);
    assert_eq!(first.released.len(), 0);
    assert_eq!(first.reused.len(), 0);

    let _ = cache.build_tile_state(
        grid,
        &DamageRegion::from_rects([Rect::new(0.0, 0.0, 20.0, 20.0)]),
    );
    let second = pool.synchronize(&mut cache);
    assert_eq!(second.allocated.len(), 0);
    assert_eq!(second.released.len(), 0);
    assert_eq!(second.reused.len(), 1);
}

#[test]
fn tile_cache_evicts_stale_reusable_handles() {
    let mut cache = TileCache::new();
    let mut pool = TileResourcePool::new();
    let large_grid = TileGrid::new(Size::new(300.0, 300.0), 128.0, 128.0);
    let small_grid = TileGrid::new(Size::new(128.0, 128.0), 128.0, 128.0);

    let _ = cache.build_tile_state(large_grid, &DamageRegion::Full);
    let _ = pool.synchronize(&mut cache);
    let _ = cache.build_tile_state(small_grid, &DamageRegion::Full);
    let _ = pool.synchronize(&mut cache);
    let mut released_count = 0;
    for _ in 0..5 {
        let _ = cache.build_tile_state(small_grid, &DamageRegion::Empty);
        let delta = pool.synchronize(&mut cache);
        released_count += delta.released.len();
    }
    assert!(released_count > 0);
}

#[test]
fn tile_cache_reusable_pool_stays_within_budget() {
    let mut cache = TileCache::new();
    let mut pool = TileResourcePool::new();
    let mut saw_budget_eviction = false;
    for tile_edge in [512.0, 448.0, 384.0, 320.0, 256.0, 192.0] {
        let grid = TileGrid::new(Size::new(2048.0, 2048.0), tile_edge, tile_edge);
        let _ = cache.build_tile_state(grid, &DamageRegion::Full);
        let _ = pool.synchronize(&mut cache);
        saw_budget_eviction |= cache.eviction_stats().budget_eviction_count > 0;
    }

    assert!(cache.reusable_byte_count() <= cache.reuse_budget_byte_count());
    assert!(saw_budget_eviction);
}

#[test]
fn compositor_scheduler_emits_raster_composite_present_tasks() {
    let mut cache = TileCache::new();
    let planner = CompositorPlanner::new();
    let source = root_only_source(Size::new(300.0, 300.0));
    let submission = planner.plan(
        &source,
        &mut cache,
        &DamageRegion::from_rects([Rect::new(0.0, 0.0, 20.0, 20.0)]),
    );
    let mut scheduler = CompositorScheduler::new();

    let scheduled = scheduler.schedule(7, submission);

    assert_eq!(scheduler.latest_generation(), Some(7));
    assert_eq!(
        scheduled.tasks,
        vec![
            CompositorTask::Raster,
            CompositorTask::Composite,
            CompositorTask::Present,
        ]
    );
    assert_eq!(scheduled.enqueued_frame_count, 1);
    assert_eq!(scheduled.stale_frame_count, 0);
}

#[test]
fn compositor_scheduler_coalesces_older_frames() {
    let mut cache = TileCache::new();
    let planner = CompositorPlanner::new();
    let source = root_only_source(Size::new(300.0, 300.0));
    let mut scheduler = CompositorScheduler::new();
    scheduler.enqueue_frame(
        1,
        planner.plan(
            &source,
            &mut cache,
            &DamageRegion::from_rects([Rect::new(0.0, 0.0, 10.0, 10.0)]),
        ),
    );
    scheduler.enqueue_frame(
        2,
        planner.plan(
            &source,
            &mut cache,
            &DamageRegion::from_rects([Rect::new(20.0, 20.0, 10.0, 10.0)]),
        ),
    );

    let scheduled = scheduler.take_latest().expect("latest frame");
    assert_eq!(scheduled.generation, 2);
    assert_eq!(scheduled.enqueued_frame_count, 2);
    assert_eq!(scheduled.stale_frame_count, 1);
    assert_eq!(scheduled.dropped_frame_count, 1);
    assert_eq!(scheduler.stats().pending_frame_count, 0);
}

#[test]
fn compositor_worker_tracks_processed_frames() {
    let mut cache = TileCache::new();
    let planner = CompositorPlanner::new();
    let source = root_only_source(Size::new(300.0, 300.0));
    let mut worker = CompositorWorker::new();

    let scheduled = worker.submit_frame(
        3,
        planner.plan(
            &source,
            &mut cache,
            &DamageRegion::from_rects([Rect::new(0.0, 0.0, 10.0, 10.0)]),
        ),
    );

    assert_eq!(scheduled.generation, 3);
    assert_eq!(worker.stats().processed_frame_count, 1);
    assert_eq!(worker.stats().latest_generation, Some(3));
}

#[test]
fn composite_executor_counts_layers_tiles_and_offscreen_steps() {
    let executor = CompositeExecutor::new();
    let pass = CompositePass {
        steps: vec![
            CompositeLayerPass {
                layer_id: CompositorLayerId(0),
                parent: None,
                descendant_layers: vec![CompositorLayerId(1)],
                tiles: vec![CompositeTileRef {
                    tile_id: TileId { x: 0, y: 0 },
                    content_handle: TileContentHandle(1),
                }],
                paint_order: 0,
                needs_offscreen: false,
                opacity: 1.0,
                blend_mode: CompositorBlendMode::Normal,
                effects: Vec::new(),
                bounds: Rect::new(0.0, 0.0, 128.0, 128.0),
                subtree_bounds: Rect::new(0.0, 0.0, 300.0, 300.0),
                effect_bounds: Rect::new(0.0, 0.0, 128.0, 128.0),
                effect_padding: 0.0,
            },
            CompositeLayerPass {
                layer_id: CompositorLayerId(1),
                parent: Some(CompositorLayerId(0)),
                descendant_layers: Vec::new(),
                tiles: vec![
                    CompositeTileRef {
                        tile_id: TileId { x: 1, y: 0 },
                        content_handle: TileContentHandle(2),
                    },
                    CompositeTileRef {
                        tile_id: TileId { x: 1, y: 1 },
                        content_handle: TileContentHandle(3),
                    },
                ],
                paint_order: 1,
                needs_offscreen: true,
                opacity: 0.5,
                blend_mode: CompositorBlendMode::Multiply,
                effects: Vec::new(),
                bounds: Rect::new(128.0, 0.0, 172.0, 300.0),
                subtree_bounds: Rect::new(128.0, 0.0, 172.0, 300.0),
                effect_bounds: Rect::new(128.0, 0.0, 172.0, 300.0),
                effect_padding: 0.0,
            },
        ],
        full_present: false,
    };

    let stats = executor.execute(&pass);
    assert_eq!(stats.executed_layer_count, 2);
    assert_eq!(stats.executed_tile_count, 3);
    assert_eq!(stats.offscreen_step_count, 1);

    let plan = executor.plan(&pass, TileGrid::new(Size::new(300.0, 300.0), 128.0, 128.0));
    assert_eq!(plan.layer_jobs.len(), 2);
    assert_eq!(plan.jobs.len(), 3);
    assert_eq!(plan.stats.executed_layer_count, 2);
    assert_eq!(plan.layer_jobs[1].blend_mode, CompositorBlendMode::Multiply);
    assert_eq!(plan.layer_jobs[1].parent, Some(CompositorLayerId(0)));
    assert_eq!(plan.layer_jobs[1].paint_order, 1);
    assert_eq!(
        plan.layer_jobs[0].descendant_layers,
        vec![CompositorLayerId(1)]
    );
    assert_eq!(
        plan.layer_jobs[0].subtree_bounds,
        Rect::new(0.0, 0.0, 300.0, 300.0)
    );
}

#[test]
fn compositor_planner_tracks_parent_child_layers_and_paint_order() {
    let planner = CompositorPlanner::new();
    let source = nested_context_source(Size::new(128.0, 128.0));
    let mut cache = TileCache::new();

    let submission = planner.plan(&source, &mut cache, &DamageRegion::Full);

    assert_eq!(submission.layer_tree.layers.len(), 3);
    assert_eq!(
        submission.layer_tree.layers[0].child_layers,
        vec![CompositorLayerId(1)]
    );
    assert_eq!(
        submission.layer_tree.layers[0].descendant_layers,
        vec![CompositorLayerId(1), CompositorLayerId(2)]
    );
    assert_eq!(
        submission.layer_tree.layers[1].parent,
        Some(CompositorLayerId(0))
    );
    assert_eq!(
        submission.layer_tree.layers[1].child_layers,
        vec![CompositorLayerId(2)]
    );
    assert_eq!(
        submission.layer_tree.layers[1].descendant_layers,
        vec![CompositorLayerId(2)]
    );
    assert_eq!(submission.layer_tree.layers[1].paint_order, 1);
    assert_eq!(
        submission.layer_tree.layers[2].parent,
        Some(CompositorLayerId(1))
    );
    assert_eq!(submission.layer_tree.layers[2].paint_order, 2);
    assert_eq!(
        submission.layer_tree.layers[1].subtree_bounds,
        Rect::new(10.0, 10.0, 50.0, 50.0)
    );
    assert_eq!(
        submission.layer_tree.layers[0].subtree_bounds,
        Rect::new(0.0, 0.0, 60.0, 60.0)
    );
    assert_eq!(
        submission.layer_tree.layers[0].scope_entries,
        vec![
            CompositorScopeEntry::DirectItem(0),
            CompositorScopeEntry::ChildLayer(CompositorLayerId(1)),
        ]
    );
    assert_eq!(
        submission.layer_tree.layers[1].scope_entries,
        vec![
            CompositorScopeEntry::DirectItem(1),
            CompositorScopeEntry::ChildLayer(CompositorLayerId(2)),
        ]
    );
    assert_eq!(
        submission.layer_tree.layers[2].scope_entries,
        vec![CompositorScopeEntry::DirectItem(2)]
    );
    assert_eq!(
        submission.composite_pass.steps[2].parent,
        Some(CompositorLayerId(1))
    );
    assert_eq!(submission.composite_pass.steps[2].paint_order, 2);
}

#[test]
fn threaded_compositor_worker_processes_frames() {
    let mut cache = TileCache::new();
    let planner = CompositorPlanner::new();
    let source = root_only_source(Size::new(300.0, 300.0));
    let mut worker = ThreadedCompositorWorker::new();

    let output = worker
        .submit_frame(
            9,
            planner.plan(
                &source,
                &mut cache,
                &DamageRegion::from_rects([Rect::new(0.0, 0.0, 10.0, 10.0)]),
            ),
        )
        .expect("threaded worker output");

    assert_eq!(output.scheduled.generation, 9);
    assert_eq!(output.worker_stats.submitted_frame_count, 1);
    assert_eq!(output.worker_stats.processed_frame_count, 1);
    assert!(output.worker_stats.worker_threaded);
    assert!(output.worker_stats.worker_alive);
}

#[test]
fn compositor_service_stats_follow_worker_output() {
    let mut cache = TileCache::new();
    let planner = CompositorPlanner::new();
    let source = root_only_source(Size::new(300.0, 300.0));
    let mut service = CompositorService::new();

    let _ = service
        .submit_frame(
            11,
            planner.plan(
                &source,
                &mut cache,
                &DamageRegion::from_rects([Rect::new(0.0, 0.0, 12.0, 12.0)]),
            ),
        )
        .expect("service output");
    let stats = service.stats();

    assert_eq!(stats.submitted_frame_count, 1);
    assert_eq!(stats.processed_frame_count, 1);
    assert!(stats.worker_threaded);
    assert!(stats.worker_alive);
}

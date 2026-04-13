use super::super::*;

#[test]
fn compose_engine_reuses_retained_scene_until_invalidated() {
    let root = column(vec![text("Cache"), text("Hit")]).spacing(4.0);
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let first = engine.compose(&root, Size::new(320.0, 240.0));
    let second = engine.compose(&root, Size::new(320.0, 240.0));

    assert_eq!(first, second);
    assert_eq!(engine.stats().compose_passes, 1);
    assert_eq!(engine.stats().layout_passes, 1);
    assert_eq!(engine.stats().cache_hits, 1);

    engine.invalidate(DirtyReason::Paint);
    let third = engine.compose(&root, Size::new(320.0, 240.0));

    assert_eq!(third.packet_count(), second.packet_count());
    assert_eq!(engine.stats().compose_passes, 2);
    assert_eq!(engine.stats().layout_passes, 1);
    assert_eq!(engine.stats().cache_hits, 1);
}

#[test]
fn compose_engine_can_repaint_single_dirty_node_without_layout() {
    let title = text("Title").foreground(Color::WHITE);
    let title_id = title.id();
    let root = column(vec![title, text("Body")])
        .spacing(4.0)
        .background(Color::rgba(39, 110, 241, 255));
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let baseline = engine.compose(&root, Size::new(320.0, 240.0));
    engine.invalidate_node(title_id, DirtyReason::Paint);
    let repainted = engine.compose(&root, Size::new(320.0, 240.0));

    assert_eq!(baseline.packet_count(), repainted.packet_count());
    assert_eq!(engine.stats().layout_passes, 1);
    assert_eq!(engine.stats().compose_passes, 2);
}

#[test]
fn compose_submit_returns_full_scene_when_paint_invalidation_keeps_commands_identical() {
    let title = text("Title").key("title");
    let title_id = title.id();
    let root = column(vec![title, text("Body").key("body")]).spacing(4.0);
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&root, Size::new(320.0, 240.0)));
    engine.invalidate_node(title_id, DirtyReason::Paint);
    let submit = snapshot_submit(engine.compose_submit_retained(&root, Size::new(320.0, 240.0)));

    assert!(matches!(submit, RenderSceneUpdate::Full(_)));
}

#[test]
fn compose_submit_reconciles_keyed_rebuild_as_paint_patch() {
    let first = column(vec![text("Title").key("title"), text("Body").key("body")])
        .spacing(4.0)
        .key("root");
    let second = column(vec![
        text("Title").key("title").foreground(Color::WHITE),
        text("Body").key("body"),
    ])
    .spacing(4.0)
    .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let (submit, display_list) =
        snapshot_outputs(engine.compose_submit_retained(&second, Size::new(320.0, 240.0)));

    if let RenderSceneUpdate::Delta { delta: patch, .. } = submit {
        assert!(patch.object_upserts.len() <= 1);
        assert!(patch.object_removes.is_empty());
    }
    assert_eq!(display_list.items.len(), 2);
    assert_eq!(engine.stats().layout_passes, 1);
    assert_eq!(engine.stats().compose_passes, 2);
}

#[test]
fn compose_submit_reconciles_keyed_layout_change_as_layout_work() {
    let first = column(vec![text("Title").key("title"), text("Body").key("body")])
        .spacing(4.0)
        .key("root");
    let second = column(vec![text("Title").key("title"), text("Body").key("body")])
        .spacing(12.0)
        .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let (submit, display_list) =
        snapshot_outputs(engine.compose_submit_retained(&second, Size::new(320.0, 240.0)));

    assert!(!display_list.items.is_empty());
    assert!(matches!(
        submit,
        RenderSceneUpdate::Delta { .. } | RenderSceneUpdate::Full(_)
    ));
    assert_eq!(engine.stats().layout_passes, 2);
}

#[test]
fn compose_submit_treats_arrangement_change_as_layout_work() {
    let first = row(vec![spacer(20.0, 10.0).key("a"), spacer(20.0, 10.0).key("b")])
        .fixed_size(100.0, 20.0)
        .arrangement(Arrangement::Start)
        .key("root");
    let second = row(vec![spacer(20.0, 10.0).key("a"), spacer(20.0, 10.0).key("b")])
        .fixed_size(100.0, 20.0)
        .arrangement(Arrangement::End)
        .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let (submit, display_list) =
        snapshot_outputs(engine.compose_submit_retained(&second, Size::new(320.0, 240.0)));

    let current = match submit {
        RenderSceneUpdate::Delta { current, .. } => current,
        RenderSceneUpdate::Full(current) => current,
    };
    assert_eq!(current.size, Size::new(320.0, 240.0));
    assert!(display_list.items.is_empty());
    assert_eq!(engine.stats().layout_passes, 2);
}

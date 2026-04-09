use super::*;

#[test]
fn builds_scene_from_column_tree() {
    let root = column(vec![
        text("Hello").font_size(20.0),
        spacer(0.0, 8.0),
        text("World"),
    ])
    .padding_all(12.0)
    .spacing(6.0)
    .background(Color::rgba(245, 247, 250, 255));

    let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);

    assert_eq!(scene.commands.len(), 3);
    assert!(matches!(scene.commands[0], DrawCommand::Fill { .. }));
    assert!(matches!(scene.commands[1], DrawCommand::Text { .. }));
    assert!(matches!(scene.commands[2], DrawCommand::Text { .. }));
}

#[test]
fn builds_scene_from_nested_container_and_row() {
    let root = container(
        row(vec![text("A"), spacer(12.0, 0.0), text("B")])
            .spacing(8.0)
            .foreground(Color::WHITE),
    )
    .padding_all(16.0)
    .background(Color::rgba(39, 110, 241, 255))
    .corner_radius(18.0);

    let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);

    assert_eq!(scene.commands.len(), 3);
    assert!(matches!(
        scene.commands[0],
        DrawCommand::Fill {
            shape: zeno_graphics::Shape::RoundedRect { .. },
            ..
        }
    ));
}

#[test]
fn keyed_nodes_keep_stable_ids_across_rebuilds() {
    let first = text("Label").key("title");
    let second = text("Label").key("title");
    let third = text("Label").key("body");

    assert_eq!(first.id(), second.id());
    assert_ne!(first.id(), third.id());
}

#[test]
fn compose_submit_keeps_text_baseline_in_sync_with_text_metrics() {
    let root = text("Baseline").font_size(20.0).padding_all(10.0);
    let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);

    match &scene.commands[0] {
        DrawCommand::Text {
            position, layout, ..
        } => {
            assert_eq!(position.y, 10.0 + layout.metrics.ascent);
            assert!(layout.metrics.ascent > 0.0);
            assert!(layout.metrics.descent >= 0.0);
        }
        _ => panic!("expected text command"),
    }
}

#[test]
fn dump_helpers_report_scene_and_layout_structure() {
    let root = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .opacity(0.5)
        .layer()
        .key("root");
    let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
    let scene_dump = dump_scene(&scene);
    let layout_dump = dump_layout(&root, Size::new(320.0, 240.0), &FallbackTextSystem);

    assert!(scene_dump.contains("layer id="));
    assert!(scene_dump.contains("blend="));
    assert!(layout_dump.contains("node id="));
    assert!(layout_dump.contains("text lines="));
}

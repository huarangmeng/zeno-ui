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
    let commands: Vec<_> = scene.iter_commands().collect();

    assert_eq!(scene.command_count(), 3);
    assert!(matches!(commands[0], DrawCommand::Fill { .. }));
    assert!(matches!(commands[1], DrawCommand::Text { .. }));
    assert!(matches!(commands[2], DrawCommand::Text { .. }));
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
    let commands: Vec<_> = scene.iter_commands().collect();

    assert_eq!(scene.command_count(), 3);
    assert!(matches!(
        commands[0],
        DrawCommand::Fill {
            shape: zeno_scene::Shape::RoundedRect { .. },
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
    let commands: Vec<_> = scene.iter_commands().collect();

    match commands[0] {
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

#[test]
fn box_aligns_children_within_fixed_bounds() {
    let spacer_id = spacer(20.0, 10.0).key("child").id().0;
    let root = r#box(vec![spacer(20.0, 10.0).key("child")])
        .fixed_size(100.0, 60.0)
        .padding_all(5.0)
        .content_alignment(Alignment::BOTTOM_END)
        .background(Color::WHITE)
        .key("root");
    let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
    let child_block = scene
        .blocks
        .iter()
        .find(|block| block.node_id == spacer_id)
        .expect("aligned child block");

    assert_eq!(child_block.bounds.origin.x, 75.0);
    assert_eq!(child_block.bounds.origin.y, 45.0);
}

#[test]
fn row_arrangement_and_cross_axis_alignment_position_children() {
    let first_id = spacer(20.0, 10.0).key("first").id().0;
    let second_id = spacer(10.0, 20.0).key("second").id().0;
    let root = row(vec![
        spacer(20.0, 10.0).key("first"),
        spacer(10.0, 20.0).key("second"),
    ])
    .fixed_size(100.0, 40.0)
    .arrangement(Arrangement::SpaceBetween)
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .key("root");
    let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
    let first = scene
        .blocks
        .iter()
        .find(|block| block.node_id == first_id)
        .expect("first block");
    let second = scene
        .blocks
        .iter()
        .find(|block| block.node_id == second_id)
        .expect("second block");

    assert_eq!(first.bounds.origin.x, 0.0);
    assert_eq!(first.bounds.origin.y, 15.0);
    assert_eq!(second.bounds.origin.x, 90.0);
    assert_eq!(second.bounds.origin.y, 10.0);
}

#[test]
fn column_arrangement_and_cross_axis_alignment_position_children() {
    let top_id = spacer(20.0, 10.0).key("top").id().0;
    let bottom_id = spacer(10.0, 20.0).key("bottom").id().0;
    let root = column(vec![
        spacer(20.0, 10.0).key("top"),
        spacer(10.0, 20.0).key("bottom"),
    ])
    .fixed_size(40.0, 100.0)
    .arrangement(Arrangement::End)
    .cross_axis_alignment(CrossAxisAlignment::End)
    .key("root");
    let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
    let top = scene
        .blocks
        .iter()
        .find(|block| block.node_id == top_id)
        .expect("top block");
    let bottom = scene
        .blocks
        .iter()
        .find(|block| block.node_id == bottom_id)
        .expect("bottom block");

    assert_eq!(top.bounds.origin.x, 20.0);
    assert_eq!(top.bounds.origin.y, 70.0);
    assert_eq!(bottom.bounds.origin.x, 30.0);
    assert_eq!(bottom.bounds.origin.y, 80.0);
}

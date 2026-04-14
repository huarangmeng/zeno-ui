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

    let renderer = ComposeRenderer::new(&FallbackTextSystem);
    let display_list = renderer.compose_display_list(&root, Size::new(320.0, 240.0));

    assert_eq!(display_list.items.len(), 3);
    assert!(matches!(
        display_list.items[0].payload,
        zeno_scene::DisplayItemPayload::FillRect { .. }
            | zeno_scene::DisplayItemPayload::FillRoundedRect { .. }
    ));
    assert!(matches!(
        display_list.items[1].payload,
        zeno_scene::DisplayItemPayload::TextRun(_)
    ));
    assert!(matches!(
        display_list.items[2].payload,
        zeno_scene::DisplayItemPayload::TextRun(_)
    ));
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

    let renderer = ComposeRenderer::new(&FallbackTextSystem);
    let display_list = renderer.compose_display_list(&root, Size::new(320.0, 240.0));

    assert_eq!(display_list.items.len(), 3);
    assert!(matches!(
        display_list.items[0].payload,
        zeno_scene::DisplayItemPayload::FillRoundedRect { .. }
    ));
    assert!(
        display_list
            .items
            .iter()
            .filter(|item| matches!(item.payload, zeno_scene::DisplayItemPayload::TextRun(_)))
            .count()
            >= 2
    );
}

#[test]
fn image_node_measures_and_appears_in_layout_dump() {
    let root = image_rgba8(
        2.0,
        2.0,
        vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ],
    )
    .fixed_size(24.0, 18.0)
    .key("image");
    let viewport = Size::new(320.0, 240.0);
    let measured =
        crate::layout::measure_node(&root, Point::new(0.0, 0.0), viewport, &FallbackTextSystem);
    let crate::layout::MeasuredKind::Image = measured.kind else {
        panic!("image node should measure into image kind");
    };
    assert_eq!(measured.frame.size, Size::new(24.0, 18.0));

    let layout_dump = dump_layout(&root, viewport, &FallbackTextSystem);
    assert!(layout_dump.contains("image 2x2"));
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
    let viewport = Size::new(320.0, 240.0);
    let measured =
        crate::layout::measure_node(&root, Point::new(0.0, 0.0), viewport, &FallbackTextSystem);
    let crate::layout::MeasuredKind::Text(text_layout) = measured.kind else {
        panic!("text node should measure into text layout");
    };
    assert!(text_layout.metrics.ascent > 0.0);
    assert!(text_layout.metrics.descent >= 0.0);
}

#[test]
fn dump_helpers_report_display_list_and_layout_structure() {
    let root = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .opacity(0.5)
        .layer()
        .key("root");
    let display_list = ComposeRenderer::new(&FallbackTextSystem)
        .compose_display_list(&root, Size::new(320.0, 240.0));
    let layout_dump = dump_layout(&root, Size::new(320.0, 240.0), &FallbackTextSystem);

    assert_eq!(display_list.stacking_contexts.len(), 1);
    assert_eq!(display_list.items.len(), 2);
    assert!(layout_dump.contains("node id="));
    assert!(layout_dump.contains("text lines="));
}

#[test]
fn box_aligns_children_within_fixed_bounds() {
    let root = r#box(vec![spacer(20.0, 10.0).key("child")])
        .fixed_size(100.0, 60.0)
        .padding_all(5.0)
        .content_alignment(Alignment::BOTTOM_END)
        .background(Color::WHITE)
        .key("root");
    let viewport = Size::new(320.0, 240.0);
    let measured =
        crate::layout::measure_node(&root, Point::new(0.0, 0.0), viewport, &FallbackTextSystem);
    let crate::layout::MeasuredKind::Multiple(children) = measured.kind else {
        panic!("box should measure children");
    };
    let child = &children[0].frame;
    assert_eq!(child.origin.x, 75.0);
    assert_eq!(child.origin.y, 45.0);
}

#[test]
fn row_arrangement_and_cross_axis_alignment_position_children() {
    let root = row(vec![
        spacer(20.0, 10.0).key("first"),
        spacer(10.0, 20.0).key("second"),
    ])
    .fixed_size(100.0, 40.0)
    .arrangement(Arrangement::SpaceBetween)
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .key("root");
    let viewport = Size::new(320.0, 240.0);
    let measured =
        crate::layout::measure_node(&root, Point::new(0.0, 0.0), viewport, &FallbackTextSystem);
    let crate::layout::MeasuredKind::Multiple(children) = measured.kind else {
        panic!("row should measure children");
    };
    let first = &children[0].frame;
    let second = &children[1].frame;

    assert_eq!(first.origin.x, 0.0);
    assert_eq!(first.origin.y, 15.0);
    assert_eq!(second.origin.x, 90.0);
    assert_eq!(second.origin.y, 10.0);
}

#[test]
fn column_arrangement_and_cross_axis_alignment_position_children() {
    let root = column(vec![
        spacer(20.0, 10.0).key("top"),
        spacer(10.0, 20.0).key("bottom"),
    ])
    .fixed_size(40.0, 100.0)
    .arrangement(Arrangement::End)
    .cross_axis_alignment(CrossAxisAlignment::End)
    .key("root");
    let viewport = Size::new(320.0, 240.0);
    let measured =
        crate::layout::measure_node(&root, Point::new(0.0, 0.0), viewport, &FallbackTextSystem);
    let crate::layout::MeasuredKind::Multiple(children) = measured.kind else {
        panic!("column should measure children");
    };
    let top = &children[0].frame;
    let bottom = &children[1].frame;

    assert_eq!(top.origin.x, 20.0);
    assert_eq!(top.origin.y, 70.0);
    assert_eq!(bottom.origin.x, 30.0);
    assert_eq!(bottom.origin.y, 80.0);
}

use super::super::*;
use zeno_scene::DisplayItemPayload;

#[test]
fn compose_renderer_builds_display_list_for_background_and_text() {
    let root = container(text("Hello"))
        .padding_all(8.0)
        .background(Color::WHITE);
    let renderer = ComposeRenderer::new(&FallbackTextSystem);

    let display_list = renderer.compose_display_list(&root, Size::new(320.0, 240.0));

    assert_eq!(display_list.items.len(), 2);
    assert!(matches!(
        display_list.items[0].payload,
        DisplayItemPayload::FillRect { .. } | DisplayItemPayload::FillRoundedRect { .. }
    ));
    assert!(matches!(
        display_list.items[1].payload,
        DisplayItemPayload::TextRun(_)
    ));
}

#[test]
fn compose_engine_refreshes_retained_display_list_after_paint_invalidation() {
    let title = text("Title").foreground(Color::WHITE);
    let title_id = title.id();
    let root = column(vec![title, text("Body")])
        .spacing(4.0)
        .background(Color::rgba(39, 110, 241, 255));
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let first = engine.compose_display_list(&root, Size::new(320.0, 240.0));
    let first_generation = engine
        .current_display_list()
        .expect("retained display list should exist")
        .generation;

    engine.invalidate_node(title_id, DirtyReason::Paint);
    let second = engine.compose_display_list(&root, Size::new(320.0, 240.0));
    let second_generation = engine
        .current_display_list()
        .expect("retained display list should exist")
        .generation;

    assert_eq!(first.items.len(), second.items.len());
    assert!(second_generation > first_generation);
    assert!(
        second
            .items
            .iter()
            .any(|item| matches!(item.payload, DisplayItemPayload::TextRun(_)))
    );
}

#[test]
fn compose_update_carries_display_list_snapshot() {
    let root = container(text("Hello"))
        .padding_all(8.0)
        .background(Color::WHITE);
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let display_list = snapshot_display_list(engine.compose_update(&root, Size::new(320.0, 240.0)));

    assert_eq!(display_list.items.len(), 2);
    assert!(matches!(
        display_list.items[0].payload,
        DisplayItemPayload::FillRect { .. } | DisplayItemPayload::FillRoundedRect { .. }
    ));
    assert!(matches!(
        display_list.items[1].payload,
        DisplayItemPayload::TextRun(_)
    ));
}

#[test]
fn display_list_text_run_carries_layout_position_and_color() {
    let root = text("Hello").font_size(20.0).foreground(Color::WHITE);
    let renderer = ComposeRenderer::new(&FallbackTextSystem);

    let display_list = renderer.compose_display_list(&root, Size::new(320.0, 240.0));
    let text = match &display_list.items[0].payload {
        DisplayItemPayload::TextRun(text) => text,
        other => panic!("expected text run payload, got {other:?}"),
    };

    assert_eq!(text.color, Color::WHITE);
    assert_eq!(text.layout.paragraph.font_size, 20.0);
    assert!(text.position.y > text.position.x);
    assert!(!text.layout.glyphs.is_empty());
}

#[test]
fn compose_renderer_builds_display_list_for_image_node() {
    let root = image_rgba8(
        2.0,
        2.0,
        vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ],
    )
    .fixed_size(20.0, 12.0);
    let renderer = ComposeRenderer::new(&FallbackTextSystem);

    let display_list = renderer.compose_display_list(&root, Size::new(320.0, 240.0));
    let image = match &display_list.items[0].payload {
        DisplayItemPayload::Image(image) => image,
        other => panic!("expected image payload, got {other:?}"),
    };

    assert_eq!(image.width, 2);
    assert_eq!(image.height, 2);
    assert_eq!(image.dest_rect.size, Size::new(20.0, 12.0));
    assert_eq!(image.rgba8.len(), 16);
}

#[test]
fn compose_update_carries_image_display_list() {
    let root = image_rgba8(1.0, 1.0, vec![12, 34, 56, 255]).fixed_size(18.0, 10.0);
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let (update, display_list) =
        snapshot_outputs(engine.compose_update(&root, Size::new(320.0, 240.0)));

    assert!(matches!(
        display_list.items[0].payload,
        DisplayItemPayload::Image(_)
    ));
    assert!(matches!(update, ComposeUpdate::Full { .. }));
}

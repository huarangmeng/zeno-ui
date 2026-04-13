use super::*;

#[test]
fn layer_creating_paint_change_emits_direct_layer_patch() {
    let first = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .key("root");
    let root_id = first.id().0;
    let second = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .opacity(0.5)
        .layer()
        .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let display_list = snapshot_display_list(
        engine.compose_submit_retained(&second, Size::new(320.0, 240.0)),
    );

    assert_eq!(display_list.stacking_contexts.len(), 1);
    let context = &display_list.stacking_contexts[0];
    assert_eq!(context.opacity, 0.5);
    assert!(context.needs_offscreen);
    let _ = root_id;
}

#[test]
fn adding_layer_rehomes_descendant_blocks_in_patch() {
    let first = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .key("root");
    let root_id = first.id().0;
    let text_id = text("Hello").key("text").id().0;
    let second = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .opacity(0.5)
        .layer()
        .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let display_list = snapshot_display_list(
        engine.compose_submit_retained(&second, Size::new(320.0, 240.0)),
    );

    assert_eq!(display_list.stacking_contexts.len(), 1);
    let text_item = display_list
        .items
        .iter()
        .find(|item| matches!(item.payload, zeno_scene::DisplayItemPayload::TextRun(_)))
        .expect("text item");
    assert!(text_item.stacking_context.is_some());
    let _ = (root_id, text_id);
}

#[test]
fn removing_layer_rehomes_descendant_blocks_in_patch() {
    let first = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .opacity(0.5)
        .layer()
        .key("root");
    let text_id = text("Hello").key("text").id().0;
    let second = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let display_list = snapshot_display_list(
        engine.compose_submit_retained(&second, Size::new(320.0, 240.0)),
    );

    let text_item = display_list
        .items
        .iter()
        .find(|item| matches!(item.payload, zeno_scene::DisplayItemPayload::TextRun(_)))
        .expect("text item");
    assert!(text_item.stacking_context.is_none());
    assert!(display_list.stacking_contexts.is_empty());
    let _ = text_id;
}

#[test]
fn layer_effect_change_emits_direct_layer_upsert() {
    let first = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .layer()
        .key("root");
    let root_id = first.id().0;
    let second = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .layer()
        .blend_mode(BlendMode::Multiply)
        .drop_shadow(4.0, 6.0, 8.0, Color::rgba(0, 0, 0, 120))
        .key("root");
    let mut engine = ComposeEngine::new(&FallbackTextSystem);

    let _ = snapshot_submit(engine.compose_submit_retained(&first, Size::new(320.0, 240.0)));
    let display_list = snapshot_display_list(
        engine.compose_submit_retained(&second, Size::new(320.0, 240.0)),
    );

    let context = display_list.stacking_contexts.first().expect("stacking context");
    assert_eq!(context.blend_mode, zeno_scene::BlendMode::Multiply);
    assert_eq!(
        context.effects,
        vec![zeno_scene::Effect::DropShadow {
            dx: 4.0,
            dy: 6.0,
            blur: 8.0,
            color: Color::rgba(0, 0, 0, 120),
        }]
    );
    assert!(context.needs_offscreen);
    let _ = root_id;
}

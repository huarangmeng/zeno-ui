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

    let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
    let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

    match submit {
        SceneSubmit::Patch { patch, current } => {
            assert!(
                patch
                    .layer_upserts
                    .iter()
                    .any(|layer| layer.layer_id == root_id)
            );
            assert!(patch.layer_removes.is_empty());
            assert!(patch.upserts.iter().any(|block| block.node_id == root_id));
            assert!(current.layers.iter().any(|layer| layer.layer_id == root_id));
        }
        SceneSubmit::Full(_) => panic!("expected patch submit"),
    }
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

    let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
    let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

    match submit {
        SceneSubmit::Patch { patch, current } => {
            let text_block = patch
                .upserts
                .iter()
                .find(|block| block.node_id == text_id)
                .expect("text block upsert");
            assert_eq!(text_block.layer_id, root_id);
            let current_text_block = current
                .blocks
                .iter()
                .find(|block| block.node_id == text_id)
                .expect("current text block");
            assert_eq!(current_text_block.layer_id, root_id);
        }
        SceneSubmit::Full(_) => panic!("expected patch submit"),
    }
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

    let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
    let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

    match submit {
        SceneSubmit::Patch { patch, current } => {
            let text_block = patch
                .upserts
                .iter()
                .find(|block| block.node_id == text_id)
                .expect("text block upsert");
            assert_eq!(text_block.layer_id, Scene::ROOT_LAYER_ID);
            let current_text_block = current
                .blocks
                .iter()
                .find(|block| block.node_id == text_id)
                .expect("current text block");
            assert_eq!(current_text_block.layer_id, Scene::ROOT_LAYER_ID);
        }
        SceneSubmit::Full(_) => panic!("expected patch submit"),
    }
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

    let _ = engine.compose_submit(&first, Size::new(320.0, 240.0));
    let submit = engine.compose_submit(&second, Size::new(320.0, 240.0));

    match submit {
        SceneSubmit::Patch { patch, .. } => {
            let layer = patch
                .layer_upserts
                .iter()
                .find(|layer| layer.layer_id == root_id)
                .expect("layer upsert");
            assert_eq!(layer.blend_mode, SceneBlendMode::Multiply);
            assert_eq!(
                layer.effects,
                vec![SceneEffect::DropShadow {
                    dx: 4.0,
                    dy: 6.0,
                    blur: 8.0,
                    color: Color::rgba(0, 0, 0, 120),
                }]
            );
            assert!(layer.offscreen);
        }
        SceneSubmit::Full(_) => panic!("expected patch submit"),
    }
}

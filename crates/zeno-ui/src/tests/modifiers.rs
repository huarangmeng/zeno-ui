use super::*;

#[test]
fn modifier_chain_resolves_same_style_as_legacy_builder_api() {
    let via_builder = text("Hello")
        .padding_all(12.0)
        .background(Color::WHITE)
        .foreground(Color::rgba(10, 20, 30, 255))
        .font_size(18.0)
        .corner_radius(8.0)
        .fixed_size(120.0, 40.0);
    let via_modifiers = text("Hello").modifiers([
        Modifier::Padding(EdgeInsets::all(12.0)),
        Modifier::Background(Color::WHITE),
        Modifier::Foreground(Color::rgba(10, 20, 30, 255)),
        Modifier::FontSize(18.0),
        Modifier::CornerRadius(8.0),
        Modifier::FixedSize {
            width: 120.0,
            height: 40.0,
        },
    ]);

    assert_eq!(via_builder.resolved_style(), via_modifiers.resolved_style());
    assert_eq!(
        via_builder.modifiers.resolve_style(),
        via_modifiers.resolved_style()
    );
}

#[test]
fn fixed_size_modifier_overrides_individual_size_resolution() {
    let node = spacer(10.0, 12.0)
        .width(40.0)
        .height(50.0)
        .fixed_size(120.0, 80.0);

    assert_eq!(node.resolved_style().width, Some(120.0));
    assert_eq!(node.resolved_style().height, Some(80.0));
}

#[test]
fn content_alignment_modifier_resolves_into_style() {
    let node = r#box(vec![spacer(10.0, 10.0)])
        .fixed_size(120.0, 80.0)
        .content_alignment(Alignment::BOTTOM_END);

    assert_eq!(node.resolved_style().content_alignment, Alignment::BOTTOM_END);
}

#[test]
fn stack_layout_modifiers_resolve_into_style() {
    let node = row(vec![spacer(10.0, 10.0), spacer(10.0, 10.0)])
        .arrangement(Arrangement::SpaceBetween)
        .cross_axis_alignment(CrossAxisAlignment::Center);

    assert_eq!(node.resolved_style().arrangement, Arrangement::SpaceBetween);
    assert_eq!(
        node.resolved_style().cross_axis_alignment,
        CrossAxisAlignment::Center
    );
}

#[test]
fn font_size_modifier_drives_text_layout_metrics() {
    let scene = compose_scene(
        &text("Hello").modifier(Modifier::FontSize(24.0)),
        Size::new(320.0, 240.0),
        &FallbackTextSystem,
    );
    let commands: Vec<_> = scene.iter_commands().collect();

    match commands[0] {
        DrawCommand::Text { layout, .. } => {
            assert_eq!(layout.paragraph.font_size, 24.0);
        }
        _ => panic!("expected text command"),
    }
}

#[test]
fn modifier_api_builds_same_scene_as_legacy_style_api() {
    let via_builder = container(text("Hello").foreground(Color::WHITE).key("text"))
        .key("root")
        .padding_all(12.0)
        .background(Color::rgba(39, 110, 241, 255))
        .corner_radius(18.0);
    let via_modifier = container(
        text("Hello")
            .modifier(Modifier::Foreground(Color::WHITE))
            .key("text"),
    )
    .key("root")
    .modifiers([
        Modifier::Padding(EdgeInsets::all(12.0)),
        Modifier::Background(Color::rgba(39, 110, 241, 255)),
        Modifier::CornerRadius(18.0),
    ]);

    let builder_scene = compose_scene(&via_builder, Size::new(320.0, 240.0), &FallbackTextSystem);
    let modifier_scene = compose_scene(&via_modifier, Size::new(320.0, 240.0), &FallbackTextSystem);

    assert_eq!(builder_scene, modifier_scene);
}

#[test]
fn clip_and_transform_modifiers_emit_structured_scene_state() {
    let root = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .clip_rounded(12.0)
        .translate(16.0, 24.0)
        .key("root");
    let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
    let layer = scene
        .layers
        .iter()
        .find(|layer| layer.layer_id == root.id().0)
        .expect("layer");
    let block = &scene.blocks[0];

    assert_eq!(layer.transform, Transform2D::translation(16.0, 24.0));
    assert_eq!(
        layer.clip,
        Some(SceneClip::RoundedRect {
            rect: layer.local_bounds,
            radius: 12.0,
        })
    );
    assert_eq!(block.bounds, layer.transform.map_rect(layer.local_bounds));
}

#[test]
fn scale_and_rotate_modifiers_emit_affine_transform_bounds() {
    let root = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .scale(2.0, 1.5)
        .rotate_degrees(90.0)
        .key("root");
    let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
    let layer = scene
        .layers
        .iter()
        .find(|layer| layer.layer_id == root.id().0)
        .expect("layer");
    let expected_transform = Transform2D::scale(2.0, 1.5).then(Transform2D::rotation_degrees(90.0));

    assert_eq!(layer.transform, expected_transform);
    assert_eq!(
        layer.bounds,
        expected_transform.map_rect(layer.local_bounds)
    );
}

#[test]
fn transform_origin_changes_affine_transform_pivot() {
    let root = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .transform_origin(0.5, 0.5)
        .rotate_degrees(90.0)
        .key("root");
    let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
    let layer = scene
        .layers
        .iter()
        .find(|layer| layer.layer_id != Scene::ROOT_LAYER_ID)
        .expect("layer");
    let pivot = Transform2D::translation(
        -layer.local_bounds.size.width * 0.5,
        -layer.local_bounds.size.height * 0.5,
    )
    .then(Transform2D::rotation_degrees(90.0))
    .then(Transform2D::translation(
        layer.local_bounds.size.width * 0.5,
        layer.local_bounds.size.height * 0.5,
    ));

    assert_eq!(layer.transform, pivot);
}

#[test]
fn opacity_and_layer_modifiers_create_compositor_layer() {
    let root = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .opacity(0.5)
        .layer()
        .key("root");
    let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
    let layer = scene
        .layers
        .iter()
        .find(|layer| layer.layer_id == root.id().0)
        .expect("opacity layer");

    assert_eq!(layer.opacity, 0.5);
    assert!(layer.offscreen);
    assert!(
        scene
            .blocks
            .iter()
            .all(|block| block.layer_id == root.id().0)
    );
}

#[test]
fn effect_modifiers_emit_layer_blend_and_effect_stack() {
    let root = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .blend_mode(BlendMode::Multiply)
        .blur(6.0)
        .drop_shadow(4.0, 6.0, 8.0, Color::rgba(0, 0, 0, 120))
        .key("root");
    let scene = compose_scene(&root, Size::new(320.0, 240.0), &FallbackTextSystem);
    let layer = scene
        .layers
        .iter()
        .find(|layer| layer.layer_id == root.id().0)
        .expect("effect layer");

    assert_eq!(layer.blend_mode, SceneBlendMode::Multiply);
    assert_eq!(
        layer.effects,
        vec![
            SceneEffect::Blur { sigma: 6.0 },
            SceneEffect::DropShadow {
                dx: 4.0,
                dy: 6.0,
                blur: 8.0,
                color: Color::rgba(0, 0, 0, 120),
            },
        ]
    );
    assert_eq!(
        layer.bounds,
        zeno_core::Rect::new(
            layer.local_bounds.origin.x - 38.0,
            layer.local_bounds.origin.y - 36.0,
            layer.local_bounds.size.width + 84.0,
            layer.local_bounds.size.height + 84.0,
        )
    );
    assert!(layer.offscreen);
}

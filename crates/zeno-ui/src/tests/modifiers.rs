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

    assert_eq!(
        node.resolved_style().content_alignment,
        Alignment::BOTTOM_END
    );
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
    let node = text("Hello").modifier(Modifier::FontSize(24.0));
    let viewport = Size::new(320.0, 240.0);
    let measured =
        crate::layout::measure_node(&node, Point::new(0.0, 0.0), viewport, &FallbackTextSystem);
    let crate::layout::MeasuredKind::Text(text_layout) = measured.kind else {
        panic!("text node should measure into text layout");
    };
    assert_eq!(text_layout.paragraph.font_size, 24.0);
}

#[test]
fn modifier_api_builds_same_display_list_as_legacy_style_api() {
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

    let builder_display_list = ComposeRenderer::new(&FallbackTextSystem)
        .compose_display_list(&via_builder, Size::new(320.0, 240.0));
    let modifier_display_list = ComposeRenderer::new(&FallbackTextSystem)
        .compose_display_list(&via_modifier, Size::new(320.0, 240.0));

    assert_eq!(builder_display_list, modifier_display_list);
}

#[test]
fn clip_and_transform_modifiers_emit_structured_display_list_state() {
    let root = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .clip_rounded(12.0)
        .translate(16.0, 24.0)
        .key("root");
    let display_list = ComposeRenderer::new(&FallbackTextSystem)
        .compose_display_list(&root, Size::new(320.0, 240.0));
    let spatial = display_list
        .spatial_tree
        .nodes
        .iter()
        .find(|node| node.world_transform == Transform2D::translation(16.0, 24.0))
        .expect("translated spatial node");
    let clipped_item = display_list
        .items
        .iter()
        .find(|item| {
            item.spatial_id == spatial.id && item.clip_chain_id != zeno_scene::ClipChainId(0)
        })
        .expect("clipped item");
    let clip_chain = display_list
        .clip_chains
        .chains
        .iter()
        .find(|chain| chain.id == clipped_item.clip_chain_id)
        .expect("item clip chain");

    assert_eq!(
        clip_chain.clip,
        ClipRegion::RoundedRect {
            rect: zeno_core::Rect::new(
                0.0,
                0.0,
                clipped_item.visual_rect.size.width,
                clipped_item.visual_rect.size.height,
            ),
            radius: 12.0,
        }
    );
    assert_eq!(clipped_item.spatial_id, spatial.id);
}

#[test]
fn scale_and_rotate_modifiers_emit_affine_transform_state() {
    let root = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .scale(2.0, 1.5)
        .rotate_degrees(90.0)
        .key("root");
    let display_list = ComposeRenderer::new(&FallbackTextSystem)
        .compose_display_list(&root, Size::new(320.0, 240.0));
    let expected_transform = Transform2D::scale(2.0, 1.5).then(Transform2D::rotation_degrees(90.0));
    let spatial = display_list
        .spatial_tree
        .nodes
        .iter()
        .find(|node| node.world_transform == expected_transform)
        .expect("transformed spatial node");
    assert_eq!(spatial.world_transform, expected_transform);
}

#[test]
fn transform_origin_changes_affine_transform_pivot() {
    let default_root = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .rotate_degrees(90.0)
        .key("root");
    let root = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .transform_origin(0.5, 0.5)
        .rotate_degrees(90.0)
        .key("root");
    let default_display_list = ComposeRenderer::new(&FallbackTextSystem)
        .compose_display_list(&default_root, Size::new(320.0, 240.0));
    let display_list = ComposeRenderer::new(&FallbackTextSystem)
        .compose_display_list(&root, Size::new(320.0, 240.0));
    let item = default_display_list
        .items
        .iter()
        .find(|item| matches!(item.payload, zeno_scene::DisplayItemPayload::TextRun(_)))
        .expect("text item");
    let default_spatial = default_display_list
        .spatial_tree
        .nodes
        .iter()
        .find(|node| node.id == item.spatial_id)
        .expect("default spatial node");
    let spatial = display_list
        .spatial_tree
        .nodes
        .iter()
        .find(|node| node.id == item.spatial_id)
        .expect("spatial node");

    assert_ne!(spatial.world_transform, default_spatial.world_transform);
    assert_eq!(
        spatial.world_transform.m11,
        default_spatial.world_transform.m11
    );
    assert_eq!(
        spatial.world_transform.m12,
        default_spatial.world_transform.m12
    );
    assert_eq!(
        spatial.world_transform.m21,
        default_spatial.world_transform.m21
    );
    assert_eq!(
        spatial.world_transform.m22,
        default_spatial.world_transform.m22
    );
}

#[test]
fn opacity_and_layer_modifiers_create_compositor_layer() {
    let root = container(text("Hello").key("text"))
        .padding_all(8.0)
        .background(Color::WHITE)
        .opacity(0.5)
        .layer()
        .key("root");
    let display_list = ComposeRenderer::new(&FallbackTextSystem)
        .compose_display_list(&root, Size::new(320.0, 240.0));
    let layer = display_list
        .stacking_contexts
        .first()
        .expect("opacity layer");

    assert_eq!(layer.opacity, 0.5);
    assert!(layer.needs_offscreen);
    assert!(
        display_list
            .items
            .iter()
            .all(|item| item.stacking_context == Some(layer.id))
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
    let display_list = ComposeRenderer::new(&FallbackTextSystem)
        .compose_display_list(&root, Size::new(320.0, 240.0));
    let layer = display_list
        .stacking_contexts
        .first()
        .expect("effect layer");

    assert_eq!(layer.blend_mode, zeno_scene::BlendMode::Multiply);
    assert_eq!(
        layer.effects,
        vec![
            Effect::Blur { sigma: 6.0 },
            Effect::DropShadow {
                dx: 4.0,
                dy: 6.0,
                blur: 8.0,
                color: Color::rgba(0, 0, 0, 120),
            },
        ]
    );
    assert!(layer.needs_offscreen);
}

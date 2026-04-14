use zeno_ui::{Axis, Color, InteractionRole, Modifier};

use crate::{
    button, checkbox, container, r#switch, row, scroll, spacer, switch_control, text,
    toggle_button,
};

fn role_of(node: &zeno_ui::Node) -> Option<InteractionRole> {
    node.modifiers.iter().find_map(|modifier| match modifier {
        Modifier::InteractionRole(role) => Some(*role),
        _ => None,
    })
}

#[test]
fn button_applies_default_visual_style_and_role() {
    let node: zeno_ui::Node = button(text("Run")).into();
    let style = node.resolved_style();

    assert_eq!(role_of(&node), Some(InteractionRole::Button));
    assert_eq!(style.background, Some(Color::rgba(39, 110, 241, 255)));
    assert_eq!(style.foreground, Color::WHITE);
    assert_eq!(style.corner_radius, 10.0);
}

#[test]
fn button_style_can_be_overridden_explicitly() {
    let node: zeno_ui::Node = button(text("Run"))
        .background(Color::BLACK)
        .foreground(Color::rgba(240, 200, 10, 255))
        .corner_radius(24.0)
        .into();
    let style = node.resolved_style();

    assert_eq!(style.background, Some(Color::BLACK));
    assert_eq!(style.foreground, Color::rgba(240, 200, 10, 255));
    assert_eq!(style.corner_radius, 24.0);
}

#[test]
fn controls_do_not_derive_node_id_from_action() {
    zeno_ui::begin_message_bindings();
    let first_button: zeno_ui::Node = button(text("Run")).on_click(1_u8).into();
    let second_button: zeno_ui::Node =
        button(text("Run again")).on_click(1_u8).into();
    let first_checkbox: zeno_ui::Node = checkbox(text("Accept"))
        .checked(true)
        .on_checked_change(|checked| checked)
        .into();
    let second_checkbox: zeno_ui::Node = checkbox(text("Accept later"))
        .checked(false)
        .on_checked_change(|checked| checked)
        .into();
    let _bindings = zeno_ui::finish_message_bindings();

    assert_ne!(first_button.id(), second_button.id());
    assert_ne!(first_checkbox.id(), second_checkbox.id());
}

#[test]
fn button_and_checkbox_can_be_disabled() {
    let button_node: zeno_ui::Node = button(text("Run")).enabled(false).into();
    let checkbox_node: zeno_ui::Node = checkbox(text("Accept"))
        .checked(true)
        .enabled(false)
        .into();

    assert_eq!(button_node.resolved_style().opacity, 0.55);
    assert_eq!(checkbox_node.resolved_style().opacity, 0.55);
}

#[test]
fn toggle_button_reflects_selected_state() {
    let selected: zeno_ui::Node = toggle_button(text("Selected").foreground(Color::WHITE))
        .selected(true)
        .into();
    let idle: zeno_ui::Node = toggle_button(text("Idle")).selected(false).into();

    assert_eq!(role_of(&selected), Some(InteractionRole::ToggleButton));
    assert_eq!(selected.resolved_style().background, Some(Color::rgba(39, 110, 241, 255)));
    assert_eq!(idle.resolved_style().background, Some(Color::rgba(226, 232, 240, 255)));
}

#[test]
fn switch_marks_scroll_and_toggle_controls_with_distinct_roles() {
    let node: zeno_ui::Node = r#switch(text("Wi-Fi")).checked(true).into();

    assert_eq!(role_of(&node), Some(InteractionRole::Switch));
    let zeno_ui::NodeKind::Stack { children, .. } = &node.kind else {
        panic!("switch should render as labeled row");
    };
    assert_eq!(children.len(), 2);
    assert_eq!(
        children[1].resolved_style().clip,
        Some(zeno_ui::ClipMode::RoundedBounds { radius: 999.0 })
    );
}

#[test]
fn checkbox_uses_row_shell_without_promoting_root_role() {
    let node: zeno_ui::Node = checkbox(text("Accept")).checked(true).into();
    let style = node.resolved_style();

    assert_eq!(role_of(&node), Some(InteractionRole::Checkbox));
    assert_eq!(style.spacing, 10.0);
}

#[test]
fn toggle_checkbox_and_switch_allow_outer_modifier_overrides() {
    let toggle_node: zeno_ui::Node = toggle_button(text("Custom").foreground(Color::WHITE))
        .selected(true)
        .background(Color::BLACK)
        .corner_radius(24.0)
        .into();
    let checkbox_node: zeno_ui::Node = checkbox(text("Accept"))
        .checked(true)
        .spacing(14.0)
        .padding_all(6.0)
        .into();
    let switch_node: zeno_ui::Node = r#switch(text("Airplane"))
        .checked(true)
        .spacing(14.0)
        .padding_all(6.0)
        .into();
    let switch_control_node: zeno_ui::Node = switch_control()
        .checked(true)
        .background(Color::BLACK)
        .corner_radius(24.0)
        .into();

    assert_eq!(toggle_node.resolved_style().background, Some(Color::BLACK));
    assert_eq!(toggle_node.resolved_style().corner_radius, 24.0);
    assert_eq!(checkbox_node.resolved_style().spacing, 14.0);
    assert_eq!(checkbox_node.resolved_style().padding, zeno_ui::EdgeInsets::all(6.0));
    assert_eq!(switch_node.resolved_style().spacing, 14.0);
    assert_eq!(switch_node.resolved_style().padding, zeno_ui::EdgeInsets::all(6.0));
    assert_eq!(switch_control_node.resolved_style().background, Some(Color::BLACK));
    assert_eq!(switch_control_node.resolved_style().corner_radius, 24.0);
}

#[test]
fn scroll_clips_and_offsets_child_by_axis() {
    let content = container(row(vec![spacer(20.0, 20.0), spacer(20.0, 20.0)])).key("content");
    let viewport = scroll(Axis::Vertical, 18.0, content.clone());

    assert_eq!(role_of(&viewport), Some(InteractionRole::Scroll));
    assert_eq!(viewport.resolved_style().clip, Some(zeno_ui::ClipMode::Bounds));

    let zeno_ui::NodeKind::Container(child) = &viewport.kind else {
        panic!("scroll viewport should wrap child in a container");
    };
    assert_eq!(child.id(), content.id());
    assert!(child.modifiers.iter().any(|modifier| matches!(
        modifier,
        Modifier::Translate { x, y } if *x == 0.0 && *y == -18.0
    )));
}

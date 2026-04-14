use zeno_ui::{
    ActionId, Alignment, Arrangement, Color, CrossAxisAlignment, EdgeInsets, InteractionRole,
    Modifier, Node, bind_toggle_message,
};

use crate::{
    containers::{container, row, spacer},
    controls::common::{control_root_methods, finalize_control_node},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ToggleButton {
    label: Node,
    selected: bool,
    enabled: bool,
    key: Option<String>,
    action: Option<ActionId>,
    root_modifiers: Vec<Modifier>,
}

impl ToggleButton {
    #[must_use]
    pub fn new(label: Node) -> Self {
        Self {
            label,
            selected: false,
            enabled: true,
            key: None,
            action: None,
            root_modifiers: Vec::new(),
        }
    }

    #[must_use]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    #[must_use]
    pub fn on_toggle<M, F>(mut self, mapper: F) -> Self
    where
        M: 'static,
        F: Fn(bool) -> M + 'static,
    {
        self.action = Some(bind_toggle_message(mapper));
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    control_root_methods!();
}

impl From<ToggleButton> for Node {
    fn from(control: ToggleButton) -> Self {
        let indicator = spacer(8.0, 8.0)
            .background(if control.selected {
                Color::WHITE
            } else {
                Color::rgba(148, 163, 184, 255)
            })
            .corner_radius(999.0);

        let node = container(
            row([indicator, control.label])
                .spacing(8.0)
                .arrangement(Arrangement::Center)
                .cross_axis_alignment(CrossAxisAlignment::Center),
        )
        .modifier(Modifier::InteractionRole(InteractionRole::ToggleButton))
        .modifier(Modifier::Checked(control.selected))
        .modifier(Modifier::Enabled(control.enabled))
        .focusable()
        .padding(EdgeInsets::horizontal_vertical(14.0, 8.0))
        .content_alignment(Alignment::CENTER)
        .foreground(if control.selected {
            Color::WHITE
        } else {
            Color::rgba(31, 41, 55, 255)
        })
        .background(if control.selected {
            Color::rgba(39, 110, 241, 255)
        } else {
            Color::rgba(226, 232, 240, 255)
        })
        .corner_radius(999.0)
        .opacity(if control.enabled { 1.0 } else { 0.55 });

        finalize_control_node(
            node,
            control.key,
            control.action,
            control.root_modifiers,
        )
    }
}

#[must_use]
pub fn toggle_button(label: impl Into<Node>) -> ToggleButton {
    ToggleButton::new(label.into())
}

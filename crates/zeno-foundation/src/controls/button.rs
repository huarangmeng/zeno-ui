use zeno_ui::{
    ActionId, Alignment, Color, EdgeInsets, InteractionRole, Modifier, Node, bind_click_message,
};

use crate::{
    containers::container,
    controls::common::{control_root_methods, finalize_control_node},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Button {
    label: Node,
    enabled: bool,
    key: Option<String>,
    action: Option<ActionId>,
    root_modifiers: Vec<Modifier>,
}

impl Button {
    #[must_use]
    pub fn new(label: Node) -> Self {
        Self {
            label,
            enabled: true,
            key: None,
            action: None,
            root_modifiers: Vec::new(),
        }
    }

    #[must_use]
    pub fn on_click<M>(mut self, message: M) -> Self
    where
        M: Clone + 'static,
    {
        self.action = Some(bind_click_message(message));
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    control_root_methods!();
}

impl From<Button> for Node {
    fn from(control: Button) -> Self {
        let node = container(control.label)
            .modifier(Modifier::InteractionRole(InteractionRole::Button))
            .modifier(Modifier::Enabled(control.enabled))
            .focusable()
            .padding(EdgeInsets::horizontal_vertical(16.0, 10.0))
            .content_alignment(Alignment::CENTER)
            .foreground(Color::WHITE)
            .background(Color::rgba(39, 110, 241, 255))
            .corner_radius(10.0)
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
pub fn button(label: impl Into<Node>) -> Button {
    Button::new(label.into())
}

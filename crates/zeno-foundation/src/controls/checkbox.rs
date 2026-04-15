use zeno_ui::{
    ActionId, Alignment, Color, CrossAxisAlignment, EdgeInsets, InteractionRole, Modifier, Node,
    bind_toggle_message,
};

use crate::{
    containers::{container, row, spacer},
    controls::common::{control_root_methods, finalize_control_node},
};

#[derive(Debug, Clone, PartialEq)]
pub struct Checkbox {
    label: Node,
    checked: bool,
    enabled: bool,
    key: Option<String>,
    action: Option<ActionId>,
    root_modifiers: Vec<Modifier>,
}

impl Checkbox {
    #[must_use]
    pub fn new(label: Node) -> Self {
        Self {
            label,
            checked: false,
            enabled: true,
            key: None,
            action: None,
            root_modifiers: Vec::new(),
        }
    }

    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    #[must_use]
    pub fn on_checked_change<M, F>(mut self, mapper: F) -> Self
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

impl From<Checkbox> for Node {
    fn from(control: Checkbox) -> Self {
        let box_mark = container(if control.checked {
            spacer(10.0, 10.0)
                .background(Color::WHITE)
                .corner_radius(3.0)
        } else {
            spacer(10.0, 10.0)
        })
        .fixed_size(18.0, 18.0)
        .padding_all(2.0)
        .content_alignment(Alignment::CENTER)
        .background(if control.checked {
            Color::rgba(39, 110, 241, 255)
        } else {
            Color::TRANSPARENT
        })
        .corner_radius(4.0)
        .clip_rounded(4.0);

        let node = row([box_mark, control.label])
            .modifier(Modifier::InteractionRole(InteractionRole::Checkbox))
            .modifier(Modifier::Checked(control.checked))
            .modifier(Modifier::Enabled(control.enabled))
            .focusable()
            .spacing(10.0)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .padding(EdgeInsets::horizontal_vertical(2.0, 2.0))
            .opacity(if control.enabled { 1.0 } else { 0.55 });

        finalize_control_node(node, control.key, control.action, control.root_modifiers)
    }
}

#[must_use]
pub fn checkbox(label: impl Into<Node>) -> Checkbox {
    Checkbox::new(label.into())
}

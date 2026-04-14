use zeno_ui::{
    ActionId, Alignment, Color, CrossAxisAlignment, EdgeInsets, InteractionRole, Modifier, Node,
    bind_toggle_message,
};

use crate::{
    containers::{container, row, spacer},
    controls::common::{control_root_methods, finalize_control_node},
};

fn switch_track(checked: bool, apply_role: bool) -> Node {
    let content_width = 40.0 - EdgeInsets::horizontal_vertical(2.0, 2.0).horizontal();
    let knob_offset = if checked {
        (content_width - 18.0).max(0.0)
    } else {
        0.0
    };
    let knob = spacer(18.0, 18.0)
        .background(Color::WHITE)
        .corner_radius(999.0)
        .translate(knob_offset, 0.0);

    let node = container(knob)
        .padding(EdgeInsets::horizontal_vertical(2.0, 2.0))
        .fixed_size(40.0, 22.0)
        .content_alignment(Alignment::CENTER_START)
        .background(if checked {
            Color::rgba(39, 110, 241, 255)
        } else {
            Color::rgba(148, 163, 184, 255)
        })
        .corner_radius(999.0)
        .clip_rounded(999.0);
    if apply_role {
        node.modifier(Modifier::InteractionRole(InteractionRole::Switch))
            .modifier(Modifier::Checked(checked))
            .focusable()
    } else {
        node
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchControl {
    checked: bool,
    enabled: bool,
    key: Option<String>,
    action: Option<ActionId>,
    root_modifiers: Vec<Modifier>,
}

impl SwitchControl {
    #[must_use]
    pub fn new() -> Self {
        Self {
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

impl From<SwitchControl> for Node {
    fn from(control: SwitchControl) -> Self {
        finalize_control_node(
            switch_track(control.checked, true)
                .modifier(Modifier::Enabled(control.enabled))
                .opacity(if control.enabled { 1.0 } else { 0.55 }),
            control.key,
            control.action,
            control.root_modifiers,
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Switch {
    label: Node,
    checked: bool,
    enabled: bool,
    key: Option<String>,
    action: Option<ActionId>,
    root_modifiers: Vec<Modifier>,
}

impl Switch {
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

impl From<Switch> for Node {
    fn from(control: Switch) -> Self {
        let node = row([control.label, switch_track(control.checked, false)])
            .spacing(10.0)
            .cross_axis_alignment(CrossAxisAlignment::Center)
            .padding(EdgeInsets::horizontal_vertical(2.0, 2.0))
            .modifier(Modifier::InteractionRole(InteractionRole::Switch))
            .modifier(Modifier::Checked(control.checked))
            .modifier(Modifier::Enabled(control.enabled))
            .focusable()
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
pub fn switch_control() -> SwitchControl {
    SwitchControl::new()
}

#[must_use]
pub fn r#switch(label: impl Into<Node>) -> Switch {
    Switch::new(label.into())
}

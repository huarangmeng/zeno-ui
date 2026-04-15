use zeno_ui::{ActionId, Modifier, Node};

pub(crate) fn finalize_control_node(
    mut node: Node,
    key: Option<String>,
    action: Option<ActionId>,
    root_modifiers: Vec<Modifier>,
) -> Node {
    if let Some(key) = key {
        node = node.key(key);
    }
    if let Some(action_id) = action {
        node = node.action(action_id);
    }
    for modifier in root_modifiers {
        node = node.modifier(modifier);
    }
    node
}

macro_rules! control_root_methods {
    () => {
        #[must_use]
        pub fn key(mut self, key: impl AsRef<str>) -> Self {
            self.key = Some(key.as_ref().to_owned());
            self
        }

        #[must_use]
        pub fn padding_all(mut self, value: f32) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::Padding(zeno_ui::EdgeInsets::all(value)));
            self
        }

        #[must_use]
        pub fn padding(mut self, padding: zeno_ui::EdgeInsets) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::Padding(padding));
            self
        }

        #[must_use]
        pub fn background(mut self, color: zeno_ui::Color) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::Background(color));
            self
        }

        #[must_use]
        pub fn foreground(mut self, color: zeno_ui::Color) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::Foreground(color));
            self
        }

        #[must_use]
        pub fn font_size(mut self, font_size: f32) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::FontSize(font_size));
            self
        }

        #[must_use]
        pub fn corner_radius(mut self, radius: f32) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::CornerRadius(radius));
            self
        }

        #[must_use]
        pub fn spacing(mut self, spacing: f32) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::Spacing(spacing));
            self
        }

        #[must_use]
        pub fn width(mut self, width: f32) -> Self {
            self.root_modifiers.push(zeno_ui::Modifier::Width(width));
            self
        }

        #[must_use]
        pub fn height(mut self, height: f32) -> Self {
            self.root_modifiers.push(zeno_ui::Modifier::Height(height));
            self
        }

        #[must_use]
        pub fn fixed_size(mut self, width: f32, height: f32) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::FixedSize { width, height });
            self
        }

        #[must_use]
        pub fn clip(mut self) -> Self {
            self.root_modifiers.push(zeno_ui::Modifier::ClipBounds);
            self
        }

        #[must_use]
        pub fn clip_rounded(mut self, radius: f32) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::ClipRounded(radius));
            self
        }

        #[must_use]
        pub fn translate(mut self, x: f32, y: f32) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::Translate { x, y });
            self
        }

        #[must_use]
        pub fn scale(mut self, x: f32, y: f32) -> Self {
            self.root_modifiers.push(zeno_ui::Modifier::Scale { x, y });
            self
        }

        #[must_use]
        pub fn scale_uniform(self, scale: f32) -> Self {
            self.scale(scale, scale)
        }

        #[must_use]
        pub fn rotate_degrees(mut self, degrees: f32) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::RotateDegrees(degrees));
            self
        }

        #[must_use]
        pub fn transform_origin(mut self, x: f32, y: f32) -> Self {
            self.root_modifiers.push(zeno_ui::Modifier::TransformOrigin(
                zeno_ui::TransformOrigin::new(x, y),
            ));
            self
        }

        #[must_use]
        pub fn content_alignment(mut self, alignment: zeno_ui::Alignment) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::ContentAlignment(alignment));
            self
        }

        #[must_use]
        pub fn arrangement(mut self, arrangement: zeno_ui::Arrangement) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::Arrangement(arrangement));
            self
        }

        #[must_use]
        pub fn cross_axis_alignment(mut self, alignment: zeno_ui::CrossAxisAlignment) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::CrossAxisAlignment(alignment));
            self
        }

        #[must_use]
        pub fn opacity(mut self, opacity: f32) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::Opacity(opacity));
            self
        }

        #[must_use]
        pub fn layer(mut self) -> Self {
            self.root_modifiers.push(zeno_ui::Modifier::Layer);
            self
        }

        #[must_use]
        pub fn blend_mode(mut self, mode: zeno_ui::BlendMode) -> Self {
            self.root_modifiers.push(zeno_ui::Modifier::BlendMode(mode));
            self
        }

        #[must_use]
        pub fn blend_multiply(self) -> Self {
            self.blend_mode(zeno_ui::BlendMode::Multiply)
        }

        #[must_use]
        pub fn blend_screen(self) -> Self {
            self.blend_mode(zeno_ui::BlendMode::Screen)
        }

        #[must_use]
        pub fn blur(mut self, sigma: f32) -> Self {
            self.root_modifiers.push(zeno_ui::Modifier::Blur(sigma));
            self
        }

        #[must_use]
        pub fn drop_shadow(mut self, dx: f32, dy: f32, blur: f32, color: zeno_ui::Color) -> Self {
            self.root_modifiers
                .push(zeno_ui::Modifier::DropShadow(zeno_ui::DropShadow::new(
                    dx, dy, blur, color,
                )));
            self
        }
    };
}

pub(crate) use control_root_methods;

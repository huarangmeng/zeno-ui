use std::{collections::VecDeque, env, time::Duration};

use zeno_core::{
    AppConfig, Backend, BackendPreference, Color, DebugConfig, Point, Rect, RendererConfig, Size,
    Transform2D, WindowConfig, zeno_session_log,
};
use zeno_foundation::{column, container, row, spacer, text};
use zeno_runtime::{App, AppFrame, AppView, run_app_with_text_system};
use zeno_scene::{
    Brush, DrawCommand, Scene, SceneBlendMode, SceneBlock, SceneClip, SceneEffect, SceneLayer,
    Shape,
};
use zeno_text::{FontDescriptor, SystemTextSystem, TextParagraph, TextSystem};
use zeno_ui::{EdgeInsets, Node};

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
use zeno_platform::{MinimalShell, Shell};

fn main() {
    let config = AppConfig {
        app_name: "minimal_app".to_string(),
        renderer: renderer_config_from_env(),
        window: WindowConfig {
            title: "Zeno Demo App".to_string(),
            size: Size::new(1440.0, 920.0),
            ..WindowConfig::default()
        },
        debug: DebugConfig {
            frame_stats: true,
            ..DebugConfig::default()
        },
        ..AppConfig::default()
    };

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        let configured_preference = config.renderer.preference.clone();
        let app = AppState::new();
        let outcome =
            run_app_with_text_system(&config, app.text_system, app).expect("app should run");
        zeno_session_log!(
            info,
            backend = ?outcome.backend,
            configured_preference = ?configured_preference,
            attempts = outcome.attempts.len(),
            "demo session summary"
        );
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = MinimalShell.create_surface(&config.window);
    }
}

fn renderer_config_from_env() -> RendererConfig {
    let Some(value) = env::var("ZENO_DEMO_BACKEND").ok() else {
        return RendererConfig::default();
    };
    let normalized = value.trim().to_ascii_lowercase();
    let preference = match normalized.as_str() {
        "auto" => BackendPreference::Auto,
        "prefer-impeller" => BackendPreference::PreferImpeller,
        "prefer-skia" => BackendPreference::PreferSkia,
        "impeller" => BackendPreference::Force(Backend::Impeller),
        "skia" => BackendPreference::Force(Backend::Skia),
        _ => BackendPreference::PreferImpeller,
    };
    RendererConfig {
        preference: preference.clone(),
        allow_fallback: !matches!(preference, BackendPreference::Force(_)),
    }
}

const VIEWPORT_PADDING: f32 = 24.0;
const HEADER_HEIGHT: f32 = 124.0;
const NAV_HEIGHT: f32 = 44.0;
const NAV_WIDTH: f32 = 150.0;
const NAV_GAP: f32 = 14.0;
const BALL_COUNT: usize = 180;
const BALL_RADIUS: f32 = 10.0;
const COMPOSE_CARD_GAP: f32 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DemoKind {
    Physics,
    Compose,
    Compositor,
}

impl DemoKind {
    fn label(self) -> &'static str {
        match self {
            Self::Physics => "Physics",
            Self::Compose => "Compose",
            Self::Compositor => "Compositor",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Physics => "Physics Playground",
            Self::Compose => "Compose Gallery",
            Self::Compositor => "Compositor Gallery",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Physics => "实时动画、碰撞、FPS 与后端切换观测",
            Self::Compose => "声明式节点树、Modifier、UiRuntime retained patch 与 keyed reorder",
            Self::Compositor => "结构化 SceneLayer / SceneBlock、clip、blend、effect 与 transform",
        }
    }

    fn all() -> [Self; 3] {
        [Self::Physics, Self::Compose, Self::Compositor]
    }

    fn animation_interval(self) -> Option<Duration> {
        match self {
            Self::Physics | Self::Compose | Self::Compositor => Some(Duration::from_millis(16)),
        }
    }
}

struct AppState {
    active_demo: DemoKind,
    fps_counter: FpsCounter,
    world: BallWorld,
    text_system: &'static SystemTextSystem,
}

impl AppState {
    fn new() -> Self {
        let text_system = Box::leak(Box::new(SystemTextSystem));
        Self {
            active_demo: DemoKind::Physics,
            fps_counter: FpsCounter::default(),
            world: BallWorld::new(BALL_COUNT),
            text_system,
        }
    }
}

impl App for AppState {
    fn render(&mut self, frame: &AppFrame) -> AppView {
        self.fps_counter.record(frame.delta);
        if let Some(pointer) = frame.pointer.position
            && frame.pointer.just_released
            && let Some(target) = hit_test_demo_button(pointer, frame.size)
        {
            self.active_demo = target;
        }
        match self.active_demo {
            DemoKind::Physics => {
                let delta_seconds = frame.delta.as_secs_f32().clamp(0.0, 1.0 / 30.0);
                self.world.step(frame.size, delta_seconds);
                AppView::Scene(build_physics_scene(
                    &self.world,
                    self.text_system,
                    frame.size,
                    frame.backend,
                    self.fps_counter.fps(),
                    self.active_demo,
                    hovered_demo(frame.pointer.position, frame.size),
                ))
            }
            DemoKind::Compose => AppView::Compose(build_compose_root(
                frame.size,
                frame.backend,
                self.fps_counter.fps(),
                frame.elapsed.as_secs_f32(),
                self.active_demo,
                hovered_demo(frame.pointer.position, frame.size),
            )),
            DemoKind::Compositor => AppView::Scene(build_compositor_scene(
                self.text_system,
                frame.size,
                frame.backend,
                self.fps_counter.fps(),
                frame.elapsed.as_secs_f32(),
                self.active_demo,
                hovered_demo(frame.pointer.position, frame.size),
            )),
        }
    }

    fn animation_interval(&self, _frame: &AppFrame) -> Option<Duration> {
        self.active_demo.animation_interval()
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
struct FpsCounter {
    deltas: VecDeque<f32>,
    total: f32,
}

impl FpsCounter {
    fn record(&mut self, delta: Duration) {
        let seconds = delta.as_secs_f32();
        if seconds <= 0.0 {
            return;
        }
        self.deltas.push_back(seconds);
        self.total += seconds;
        while self.deltas.len() > 90 {
            if let Some(removed) = self.deltas.pop_front() {
                self.total -= removed;
            }
        }
    }

    fn fps(&self) -> f32 {
        if self.total <= 0.0 {
            0.0
        } else {
            self.deltas.len() as f32 / self.total
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Vec2 {
    x: f32,
    y: f32,
}

impl Vec2 {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Ball {
    position: Vec2,
    velocity: Vec2,
    radius: f32,
    color: Color,
}

#[derive(Debug, Clone, PartialEq)]
struct BallWorld {
    balls: Vec<Ball>,
}

impl BallWorld {
    fn new(count: usize) -> Self {
        let mut balls = Vec::with_capacity(count);
        let columns = ((count as f32).sqrt().ceil() as usize).max(1);
        let gap = BALL_RADIUS * 2.6;
        let start = BALL_RADIUS + 8.0;
        for index in 0..count {
            let row = index / columns;
            let column = index % columns;
            let speed_x = 148.0 + (index % 7) as f32 * 24.0;
            let speed_y = 164.0 + (index % 5) as f32 * 28.0;
            balls.push(Ball {
                position: Vec2::new(start + column as f32 * gap, start + row as f32 * gap),
                velocity: Vec2::new(
                    if index % 2 == 0 { speed_x } else { -speed_x },
                    if index % 3 == 0 { speed_y } else { -speed_y },
                ),
                radius: BALL_RADIUS,
                color: ball_color(index),
            });
        }
        Self { balls }
    }

    fn step(&mut self, viewport: Size, delta_seconds: f32) {
        if delta_seconds <= 0.0 {
            return;
        }
        let arena = content_rect(viewport);
        for ball in &mut self.balls {
            ball.position.x += ball.velocity.x * delta_seconds;
            ball.position.y += ball.velocity.y * delta_seconds;
            bounce_against_bounds(ball, arena);
        }

        for left in 0..self.balls.len() {
            for right in (left + 1)..self.balls.len() {
                let (first, second) = self.balls.split_at_mut(right);
                resolve_pair_collision(&mut first[left], &mut second[0]);
            }
        }
    }
}

fn build_physics_scene(
    world: &BallWorld,
    text_system: &dyn TextSystem,
    viewport: Size,
    backend: Backend,
    fps: f32,
    active_demo: DemoKind,
    hover_demo: Option<DemoKind>,
) -> Scene {
    let mut commands = chrome_commands(
        text_system,
        viewport,
        active_demo,
        hover_demo,
        backend,
        fps,
        "Physics Playground",
        "实时动画、粒子碰撞、FPS 与后端选择观测",
    );
    let arena = content_rect(viewport);
    commands.push(DrawCommand::Fill {
        shape: Shape::RoundedRect {
            rect: arena,
            radius: 28.0,
        },
        brush: Brush::Solid(Color::rgba(16, 22, 34, 255)),
    });
    for ball in &world.balls {
        let diameter = ball.radius * 2.0;
        commands.push(DrawCommand::Fill {
            shape: Shape::RoundedRect {
                rect: Rect::new(
                    ball.position.x - ball.radius,
                    ball.position.y - ball.radius,
                    diameter,
                    diameter,
                ),
                radius: ball.radius,
            },
            brush: Brush::Solid(ball.color),
        });
    }
    push_text(
        &mut commands,
        text_system,
        Point::new(arena.origin.x + 24.0, arena.origin.y + 28.0),
        18.0,
        Color::rgba(214, 225, 255, 255),
        &format!("{BALL_COUNT} balls  |  click tabs to switch demo"),
        420.0,
    );
    Scene::from_blocks(
        viewport,
        Some(Color::rgba(9, 13, 21, 255)),
        vec![SceneBlock::new(
            1,
            Scene::ROOT_LAYER_ID,
            1,
            Rect::new(0.0, 0.0, viewport.width, viewport.height),
            Transform2D::identity(),
            None,
            commands,
        )],
    )
}

fn build_compose_root(
    viewport: Size,
    backend: Backend,
    fps: f32,
    elapsed: f32,
    active_demo: DemoKind,
    hover_demo: Option<DemoKind>,
) -> Node {
    let content_width = (viewport.width - VIEWPORT_PADDING * 2.0).max(840.0);
    let card_width = ((content_width - COMPOSE_CARD_GAP) * 0.5).max(320.0);
    let pulse = 1.0 + (elapsed * 2.2).sin() * 0.04;
    let hero_rotation = (elapsed * 0.85).sin() * 3.5;
    let effect_blur = 2.0 + ((elapsed * 1.4).sin() + 1.0) * 1.5;
    let effect_opacity = 0.72 + ((elapsed * 1.1).cos() + 1.0) * 0.08;
    let order_shift = ((elapsed * 1.5) as usize) % 4;
    let submit_mode = "internal";
    let chips = reordered_labels(order_shift)
        .into_iter()
        .map(chip_node)
        .collect::<Vec<_>>();
    let nav = demo_nav_row(viewport, active_demo, hover_demo);
    container(
        column(vec![
            nav,
            hero_card(card_width, backend, fps, submit_mode, pulse, hero_rotation),
            row(vec![
                modifier_card(card_width, effect_blur, effect_opacity),
                reorder_card(card_width, chips),
            ])
            .spacing(COMPOSE_CARD_GAP)
            .key("compose-row-1"),
            row(vec![
                text_card(card_width, elapsed),
                backend_card(card_width, backend, fps, submit_mode),
            ])
            .spacing(COMPOSE_CARD_GAP)
            .key("compose-row-2"),
        ])
        .spacing(COMPOSE_CARD_GAP)
        .key("compose-content"),
    )
    .key("compose-root")
    .padding(EdgeInsets::horizontal_vertical(
        VIEWPORT_PADDING,
        VIEWPORT_PADDING,
    ))
    .background(Color::rgba(10, 14, 24, 255))
    .width(viewport.width)
    .height(viewport.height)
}

fn build_compositor_scene(
    text_system: &dyn TextSystem,
    viewport: Size,
    backend: Backend,
    fps: f32,
    elapsed: f32,
    active_demo: DemoKind,
    hover_demo: Option<DemoKind>,
) -> Scene {
    let panel_size = Size::new(400.0, 260.0);
    let orbit_size = Size::new(280.0, 280.0);
    let stream_size = Size::new(420.0, 260.0);
    let panel_position = Point::new(VIEWPORT_PADDING, HEADER_HEIGHT + VIEWPORT_PADDING + 20.0);
    let orbit_position = Point::new(viewport.width * 0.5 - orbit_size.width * 0.5, 250.0);
    let stream_position = Point::new(
        viewport.width - stream_size.width - VIEWPORT_PADDING,
        viewport.height - stream_size.height - VIEWPORT_PADDING,
    );
    let panel_transform = translated_transform(panel_position);
    let orbit_transform = centered_transform(
        orbit_position,
        orbit_size,
        (elapsed * 32.0).sin() * 10.0,
        0.96 + (elapsed * 1.7).sin() * 0.04,
    );
    let stream_transform = translated_transform(stream_position);
    let panel_bounds =
        panel_transform.map_rect(Rect::new(0.0, 0.0, panel_size.width, panel_size.height));
    let orbit_bounds =
        orbit_transform.map_rect(Rect::new(0.0, 0.0, orbit_size.width, orbit_size.height));
    let stream_bounds =
        stream_transform.map_rect(Rect::new(0.0, 0.0, stream_size.width, stream_size.height));
    let layers = vec![
        SceneLayer::root(viewport),
        SceneLayer::new(
            100,
            100,
            Some(Scene::ROOT_LAYER_ID),
            1,
            Rect::new(0.0, 0.0, panel_size.width, panel_size.height),
            panel_bounds,
            panel_transform,
            Some(SceneClip::RoundedRect {
                rect: Rect::new(0.0, 0.0, panel_size.width, panel_size.height),
                radius: 28.0,
            }),
            0.96,
            SceneBlendMode::Normal,
            vec![SceneEffect::DropShadow {
                dx: 0.0,
                dy: 16.0,
                blur: 28.0,
                color: Color::rgba(15, 25, 52, 140),
            }],
            true,
        ),
        SceneLayer::new(
            200,
            200,
            Some(Scene::ROOT_LAYER_ID),
            2,
            Rect::new(0.0, 0.0, orbit_size.width, orbit_size.height),
            orbit_bounds,
            orbit_transform,
            None,
            0.9,
            SceneBlendMode::Screen,
            vec![SceneEffect::Blur { sigma: 2.0 }],
            true,
        ),
        SceneLayer::new(
            300,
            300,
            Some(Scene::ROOT_LAYER_ID),
            3,
            Rect::new(0.0, 0.0, stream_size.width, stream_size.height),
            stream_bounds,
            stream_transform,
            Some(SceneClip::RoundedRect {
                rect: Rect::new(0.0, 0.0, stream_size.width, stream_size.height),
                radius: 26.0,
            }),
            0.95,
            SceneBlendMode::Multiply,
            vec![SceneEffect::DropShadow {
                dx: 0.0,
                dy: 12.0,
                blur: 20.0,
                color: Color::rgba(18, 28, 54, 120),
            }],
            true,
        ),
    ];
    let blocks = vec![
        SceneBlock::new(
            1,
            Scene::ROOT_LAYER_ID,
            1,
            Rect::new(0.0, 0.0, viewport.width, viewport.height),
            Transform2D::identity(),
            None,
            background_commands(viewport),
        ),
        SceneBlock::new(
            2,
            Scene::ROOT_LAYER_ID,
            2,
            Rect::new(
                VIEWPORT_PADDING,
                VIEWPORT_PADDING,
                viewport.width - VIEWPORT_PADDING * 2.0,
                HEADER_HEIGHT,
            ),
            Transform2D::identity(),
            None,
            chrome_commands(
                text_system,
                viewport,
                active_demo,
                hover_demo,
                backend,
                fps,
                "Compositor Gallery",
                "结构化 SceneLayer / SceneBlock、clip、blend、effect 与 transform",
            ),
        ),
        SceneBlock::new(
            3,
            100,
            3,
            panel_bounds,
            Transform2D::identity(),
            None,
            glass_panel_commands(text_system, panel_size, backend, fps),
        ),
        SceneBlock::new(
            4,
            200,
            4,
            orbit_bounds,
            Transform2D::identity(),
            None,
            orbit_commands(text_system, orbit_size, elapsed),
        ),
        SceneBlock::new(
            5,
            300,
            5,
            stream_bounds,
            Transform2D::identity(),
            None,
            stream_commands(text_system, stream_size, elapsed),
        ),
    ];
    Scene::from_layers_and_blocks(viewport, Some(Color::rgba(7, 10, 18, 255)), layers, blocks)
}

fn chrome_commands(
    text_system: &dyn TextSystem,
    viewport: Size,
    active_demo: DemoKind,
    hover_demo: Option<DemoKind>,
    backend: Backend,
    fps: f32,
    title: &str,
    subtitle: &str,
) -> Vec<DrawCommand> {
    let mut commands = vec![
        DrawCommand::Fill {
            shape: Shape::Rect(Rect::new(0.0, 0.0, viewport.width, viewport.height)),
            brush: Brush::Solid(Color::rgba(9, 13, 21, 255)),
        },
        DrawCommand::Fill {
            shape: Shape::RoundedRect {
                rect: Rect::new(
                    VIEWPORT_PADDING,
                    VIEWPORT_PADDING,
                    viewport.width - VIEWPORT_PADDING * 2.0,
                    HEADER_HEIGHT,
                ),
                radius: 26.0,
            },
            brush: Brush::Solid(Color::rgba(28, 34, 52, 228)),
        },
    ];
    push_text(
        &mut commands,
        text_system,
        Point::new(VIEWPORT_PADDING + 22.0, VIEWPORT_PADDING + 22.0),
        32.0,
        Color::WHITE,
        title,
        420.0,
    );
    push_text(
        &mut commands,
        text_system,
        Point::new(VIEWPORT_PADDING + 22.0, VIEWPORT_PADDING + 58.0),
        17.0,
        Color::rgba(210, 220, 248, 255),
        subtitle,
        760.0,
    );
    push_text(
        &mut commands,
        text_system,
        Point::new(viewport.width - 380.0, VIEWPORT_PADDING + 24.0),
        17.0,
        Color::rgba(168, 255, 208, 255),
        &format!("Backend {:?}  |  FPS {:.1}", backend, fps),
        340.0,
    );
    push_text(
        &mut commands,
        text_system,
        Point::new(viewport.width - 520.0, VIEWPORT_PADDING + 56.0),
        15.0,
        Color::rgba(196, 206, 231, 255),
        "点击按钮切换 demo，仍支持 ZENO_DEMO_BACKEND 环境变量",
        480.0,
    );
    for demo in DemoKind::all() {
        let rect = demo_button_rect(viewport, demo);
        let background = if active_demo == demo {
            Color::rgba(84, 122, 255, 255)
        } else if hover_demo == Some(demo) {
            Color::rgba(66, 82, 124, 255)
        } else {
            Color::rgba(49, 58, 82, 255)
        };
        commands.push(DrawCommand::Fill {
            shape: Shape::RoundedRect { rect, radius: 16.0 },
            brush: Brush::Solid(background),
        });
        push_text(
            &mut commands,
            text_system,
            Point::new(rect.origin.x + 16.0, rect.origin.y + 12.0),
            16.0,
            Color::WHITE,
            demo.label(),
            rect.size.width - 32.0,
        );
    }
    commands
}

fn demo_nav_row(
    viewport: Size,
    active_demo: DemoKind,
    hover_demo: Option<DemoKind>,
) -> Node {
    let buttons = DemoKind::all()
        .into_iter()
        .map(|demo| {
            let background = if active_demo == demo {
                Color::rgba(84, 122, 255, 255)
            } else if hover_demo == Some(demo) {
                Color::rgba(66, 82, 124, 255)
            } else {
                Color::rgba(49, 58, 82, 255)
            };
            container(
                text(demo.label())
                    .key(format!("nav-label-{}", demo.label()))
                    .font_size(16.0)
                    .foreground(Color::WHITE),
            )
            .key(format!("nav-{}", demo.label()))
            .padding(EdgeInsets::horizontal_vertical(18.0, 12.0))
            .background(background)
            .corner_radius(16.0)
            .width(NAV_WIDTH)
            .height(NAV_HEIGHT)
        })
        .collect::<Vec<_>>();
    let title_width =
        (viewport.width - VIEWPORT_PADDING * 2.0 - NAV_WIDTH * 3.0 - NAV_GAP * 2.0).max(280.0);
    row(vec![
        container(
            column(vec![
                text(DemoKind::Compose.title())
                    .key("compose-screen-title")
                    .font_size(32.0)
                    .foreground(Color::WHITE),
                text(DemoKind::Compose.subtitle())
                    .key("compose-screen-subtitle")
                    .font_size(17.0)
                    .foreground(Color::rgba(210, 220, 248, 255)),
            ])
            .spacing(8.0)
            .key("compose-screen-copy"),
        )
        .key("compose-screen-copy-card")
        .width(title_width),
        row(buttons).spacing(NAV_GAP).key("compose-nav-buttons"),
    ])
    .spacing(24.0)
    .key("compose-nav-row")
}

fn hero_card(
    width: f32,
    backend: Backend,
    fps: f32,
    submit_mode: &str,
    pulse: f32,
    hero_rotation: f32,
) -> Node {
    let accent = Color::rgba(114, 229, 255, 255);
    container(
        column(vec![
            text("Compose Gallery")
                .key("hero-title")
                .font_size(34.0)
                .foreground(Color::WHITE),
            text("展示声明式节点树、Modifier 解析、retained patch、动画与后端选择。")
                .key("hero-subtitle")
                .font_size(18.0)
                .foreground(Color::rgba(208, 220, 255, 255)),
            spacer(0.0, 12.0).key("hero-gap"),
            row(vec![
                metric_pill("Backend", &format!("{backend:?}"), accent),
                metric_pill("FPS", &format!("{fps:.1}"), Color::rgba(146, 255, 191, 255)),
                metric_pill("Submit", submit_mode, Color::rgba(255, 224, 138, 255)),
            ])
            .spacing(12.0)
            .key("hero-pills"),
        ])
        .spacing(0.0)
        .key("hero-content"),
    )
    .key("hero-card")
    .padding(EdgeInsets::horizontal_vertical(24.0, 22.0))
    .background(Color::rgba(28, 54, 118, 255))
    .corner_radius(28.0)
    .width(width)
    .layer()
    .transform_origin(0.5, 0.5)
    .scale_uniform(pulse)
    .rotate_degrees(hero_rotation)
    .drop_shadow(0.0, 16.0, 24.0, Color::rgba(34, 70, 180, 160))
}

fn modifier_card(width: f32, blur: f32, opacity: f32) -> Node {
    container(
        column(vec![
            text("Modifier Stack")
                .key("modifier-title")
                .font_size(24.0)
                .foreground(Color::WHITE),
            text("padding / background / clip / transform / opacity / layer / blur / drop shadow")
                .key("modifier-body")
                .font_size(17.0)
                .foreground(Color::rgba(227, 235, 255, 255)),
            spacer(0.0, 14.0).key("modifier-gap"),
            row(vec![
                chip_node("Layer"),
                chip_node("Blur"),
                chip_node("Shadow"),
                chip_node("Screen"),
            ])
            .spacing(10.0)
            .key("modifier-chips"),
        ])
        .spacing(0.0)
        .key("modifier-content"),
    )
    .key("modifier-card")
    .padding(EdgeInsets::horizontal_vertical(22.0, 20.0))
    .background(Color::rgba(78, 48, 118, 220))
    .corner_radius(24.0)
    .clip_rounded(24.0)
    .width(width)
    .layer()
    .opacity(opacity)
    .blend_screen()
    .blur(blur)
    .drop_shadow(0.0, 12.0, 20.0, Color::rgba(44, 12, 80, 170))
}

fn reorder_card(width: f32, chips: Vec<Node>) -> Node {
    container(
        column(vec![
            text("Keyed Reorder")
                .key("reorder-title")
                .font_size(24.0)
                .foreground(Color::WHITE),
            text("固定 key 的 chip 持续重排，便于观察 order-only patch。")
                .key("reorder-body")
                .font_size(17.0)
                .foreground(Color::rgba(225, 235, 255, 255)),
            spacer(0.0, 16.0).key("reorder-gap"),
            row(chips).spacing(10.0).key("reorder-row"),
        ])
        .spacing(0.0)
        .key("reorder-content"),
    )
    .key("reorder-card")
    .padding(EdgeInsets::horizontal_vertical(22.0, 20.0))
    .background(Color::rgba(34, 82, 76, 255))
    .corner_radius(24.0)
    .width(width)
    .layer()
    .drop_shadow(0.0, 10.0, 18.0, Color::rgba(10, 40, 36, 140))
}

fn text_card(width: f32, elapsed: f32) -> Node {
    let animated_size = 18.0 + ((elapsed * 1.3).sin() + 1.0) * 5.0;
    container(
        column(vec![
            text("Text + Cache")
                .key("text-card-title")
                .font_size(24.0)
                .foreground(Color::WHITE),
            text("SystemTextSystem 负责 shaping、段落缓存与布局，本卡片持续做字号小幅振荡。")
                .key("text-card-body")
                .font_size(animated_size)
                .foreground(Color::rgba(255, 244, 204, 255)),
        ])
        .spacing(12.0)
        .key("text-card-content"),
    )
    .key("text-card")
    .padding(EdgeInsets::horizontal_vertical(22.0, 20.0))
    .background(Color::rgba(110, 72, 40, 255))
    .corner_radius(24.0)
    .width(width)
    .clip_rounded(24.0)
}

fn backend_card(width: f32, backend: Backend, fps: f32, submit_mode: &str) -> Node {
    container(
        column(vec![
            text("Backend Routing")
                .key("backend-card-title")
                .font_size(24.0)
                .foreground(Color::WHITE),
            text("runtime 解析 Impeller / Skia，shell 负责窗口与 presenter。")
                .key("backend-card-body")
                .font_size(17.0)
                .foreground(Color::rgba(220, 232, 255, 255)),
            spacer(0.0, 14.0).key("backend-card-gap"),
            text(&format!("Resolved Backend: {backend:?}"))
                .key("backend-value")
                .font_size(18.0)
                .foreground(Color::rgba(124, 234, 255, 255)),
            text(&format!("Realtime FPS: {fps:.1}  |  Submit: {submit_mode}"))
                .key("backend-fps")
                .font_size(16.0)
                .foreground(Color::rgba(180, 255, 214, 255)),
        ])
        .spacing(0.0)
        .key("backend-card-content"),
    )
    .key("backend-card")
    .padding(EdgeInsets::horizontal_vertical(22.0, 20.0))
    .background(Color::rgba(42, 56, 92, 255))
    .corner_radius(24.0)
    .width(width)
    .layer()
    .opacity(0.94)
    .drop_shadow(0.0, 10.0, 18.0, Color::rgba(18, 24, 46, 140))
}

fn metric_pill(label: &str, value: &str, color: Color) -> Node {
    container(
        column(vec![
            text(label)
                .key(format!("{label}-label"))
                .font_size(13.0)
                .foreground(Color::rgba(214, 223, 255, 255)),
            text(value)
                .key(format!("{label}-value"))
                .font_size(18.0)
                .foreground(Color::WHITE),
        ])
        .spacing(4.0)
        .key(format!("{label}-content")),
    )
    .key(format!("pill-{label}"))
    .padding(EdgeInsets::horizontal_vertical(14.0, 12.0))
    .background(color)
    .corner_radius(18.0)
}

fn chip_node(label: &'static str) -> Node {
    let background = match label {
        "Patch" => Color::rgba(86, 112, 255, 255),
        "Layer" => Color::rgba(120, 84, 255, 255),
        "Text" => Color::rgba(255, 146, 84, 255),
        "Backend" => Color::rgba(80, 198, 150, 255),
        "Blur" => Color::rgba(154, 108, 255, 255),
        "Shadow" => Color::rgba(255, 170, 86, 255),
        "Screen" => Color::rgba(82, 176, 255, 255),
        _ => Color::rgba(74, 98, 146, 255),
    };
    container(
        text(label)
            .key(format!("chip-text-{label}"))
            .font_size(15.0)
            .foreground(Color::WHITE),
    )
    .key(format!("chip-{label}"))
    .padding(EdgeInsets::horizontal_vertical(14.0, 10.0))
    .background(background)
    .corner_radius(999.0)
}

fn reordered_labels(order_shift: usize) -> Vec<&'static str> {
    let mut labels = vec!["Patch", "Layer", "Text", "Backend"];
    let len = labels.len();
    labels.rotate_left(order_shift % len);
    labels
}

fn background_commands(viewport: Size) -> Vec<DrawCommand> {
    let mut commands = vec![DrawCommand::Fill {
        shape: Shape::Rect(Rect::new(0.0, 0.0, viewport.width, viewport.height)),
        brush: Brush::Solid(Color::rgba(7, 10, 18, 255)),
    }];
    for index in 0..10 {
        let y = HEADER_HEIGHT + VIEWPORT_PADDING + index as f32 * 64.0;
        commands.push(DrawCommand::Fill {
            shape: Shape::Rect(Rect::new(
                VIEWPORT_PADDING,
                y,
                viewport.width - VIEWPORT_PADDING * 2.0,
                1.0,
            )),
            brush: Brush::Solid(Color::rgba(25, 34, 54, 255)),
        });
    }
    commands
}

fn glass_panel_commands(
    text_system: &dyn TextSystem,
    size: Size,
    backend: Backend,
    fps: f32,
) -> Vec<DrawCommand> {
    let mut commands = vec![DrawCommand::Fill {
        shape: Shape::RoundedRect {
            rect: Rect::new(0.0, 0.0, size.width, size.height),
            radius: 28.0,
        },
        brush: Brush::Solid(Color::rgba(40, 58, 104, 220)),
    }];
    commands.push(text_command(
        text_system,
        Point::new(24.0, 28.0),
        28.0,
        Color::WHITE,
        "Glass Panel",
        size.width - 48.0,
    ));
    commands.push(text_command(
        text_system,
        Point::new(24.0, 66.0),
        16.0,
        Color::rgba(213, 222, 252, 255),
        &format!("backend {:?}  |  fps {:.1}", backend, fps),
        size.width - 48.0,
    ));
    for (index, width) in [0.82f32, 0.64, 0.91, 0.57].into_iter().enumerate() {
        let y = 118.0 + index as f32 * 28.0;
        commands.push(DrawCommand::Fill {
            shape: Shape::RoundedRect {
                rect: Rect::new(24.0, y, (size.width - 48.0) * width, 16.0),
                radius: 8.0,
            },
            brush: Brush::Solid(Color::rgba(120 + index as u8 * 20, 185, 255, 255)),
        });
    }
    commands
}

fn orbit_commands(text_system: &dyn TextSystem, size: Size, elapsed: f32) -> Vec<DrawCommand> {
    let mut commands = vec![DrawCommand::Fill {
        shape: Shape::RoundedRect {
            rect: Rect::new(0.0, 0.0, size.width, size.height),
            radius: 42.0,
        },
        brush: Brush::Solid(Color::rgba(35, 18, 62, 220)),
    }];
    let offset = ((elapsed * 2.0).sin() + 1.0) * 18.0;
    let cards = [
        (
            Color::rgba(255, 155, 122, 255),
            Rect::new(34.0 + offset, 42.0, 92.0, 120.0),
        ),
        (
            Color::rgba(111, 221, 255, 255),
            Rect::new(136.0, 82.0 + offset * 0.4, 110.0, 140.0),
        ),
        (
            Color::rgba(146, 255, 184, 255),
            Rect::new(84.0, 164.0 - offset * 0.35, 122.0, 72.0),
        ),
    ];
    for (color, rect) in cards {
        commands.push(DrawCommand::Fill {
            shape: Shape::RoundedRect { rect, radius: 24.0 },
            brush: Brush::Solid(color),
        });
    }
    commands.push(text_command(
        text_system,
        Point::new(28.0, 24.0),
        22.0,
        Color::WHITE,
        "Layer Transform + Screen Blend",
        size.width - 56.0,
    ));
    commands
}

fn stream_commands(text_system: &dyn TextSystem, size: Size, elapsed: f32) -> Vec<DrawCommand> {
    let mut commands = vec![DrawCommand::Fill {
        shape: Shape::RoundedRect {
            rect: Rect::new(0.0, 0.0, size.width, size.height),
            radius: 26.0,
        },
        brush: Brush::Solid(Color::rgba(18, 42, 50, 255)),
    }];
    commands.push(text_command(
        text_system,
        Point::new(24.0, 24.0),
        24.0,
        Color::WHITE,
        "Clip + Scroll Stream",
        size.width - 48.0,
    ));
    let scroll = (elapsed * 140.0) % 220.0;
    for index in 0..9 {
        let y = 76.0 + index as f32 * 30.0 - scroll;
        commands.push(DrawCommand::Fill {
            shape: Shape::RoundedRect {
                rect: Rect::new(24.0, y, size.width - 48.0, 18.0),
                radius: 9.0,
            },
            brush: Brush::Solid(Color::rgba(
                84 + index as u8 * 10,
                190,
                170 + index as u8 * 6,
                255,
            )),
        });
    }
    commands
}

fn text_command(
    text_system: &dyn TextSystem,
    origin: Point,
    font_size: f32,
    color: Color,
    content: &str,
    max_width: f32,
) -> DrawCommand {
    let layout = text_system.layout(TextParagraph {
        text: content.to_string(),
        font: FontDescriptor::default(),
        font_size,
        max_width,
    });
    DrawCommand::Text {
        position: Point::new(origin.x, origin.y + layout.metrics.ascent),
        layout,
        color,
    }
}

fn push_text(
    commands: &mut Vec<DrawCommand>,
    text_system: &dyn TextSystem,
    origin: Point,
    font_size: f32,
    color: Color,
    content: &str,
    max_width: f32,
) {
    commands.push(text_command(
        text_system,
        origin,
        font_size,
        color,
        content,
        max_width,
    ));
}

fn content_rect(viewport: Size) -> Rect {
    Rect::new(
        VIEWPORT_PADDING,
        HEADER_HEIGHT + VIEWPORT_PADDING + 20.0,
        (viewport.width - VIEWPORT_PADDING * 2.0).max(BALL_RADIUS * 4.0),
        (viewport.height - HEADER_HEIGHT - VIEWPORT_PADDING * 2.0 - 20.0).max(BALL_RADIUS * 4.0),
    )
}

fn demo_button_rect(viewport: Size, demo: DemoKind) -> Rect {
    let index = match demo {
        DemoKind::Physics => 0.0,
        DemoKind::Compose => 1.0,
        DemoKind::Compositor => 2.0,
    };
    let right = viewport.width - VIEWPORT_PADDING - NAV_WIDTH * 3.0 - NAV_GAP * 2.0;
    Rect::new(
        right + index * (NAV_WIDTH + NAV_GAP),
        VIEWPORT_PADDING + 72.0,
        NAV_WIDTH,
        NAV_HEIGHT,
    )
}

fn hit_test_demo_button(point: Point, viewport: Size) -> Option<DemoKind> {
    DemoKind::all()
        .into_iter()
        .find(|demo| point_in_rect(point, demo_button_rect(viewport, *demo)))
}

fn hovered_demo(position: Option<Point>, viewport: Size) -> Option<DemoKind> {
    position.and_then(|point| hit_test_demo_button(point, viewport))
}

fn point_in_rect(point: Point, rect: Rect) -> bool {
    point.x >= rect.origin.x
        && point.x <= rect.right()
        && point.y >= rect.origin.y
        && point.y <= rect.bottom()
}

fn translated_transform(position: Point) -> Transform2D {
    Transform2D::translation(position.x, position.y)
}

fn centered_transform(position: Point, size: Size, rotation: f32, scale: f32) -> Transform2D {
    let pivot = Point::new(size.width * 0.5, size.height * 0.5);
    Transform2D::translation(-pivot.x, -pivot.y)
        .then(Transform2D::scale(scale, scale))
        .then(Transform2D::rotation_degrees(rotation))
        .then(Transform2D::translation(pivot.x, pivot.y))
        .then(Transform2D::translation(position.x, position.y))
}

fn bounce_against_bounds(ball: &mut Ball, arena: Rect) {
    let min_x = arena.origin.x + ball.radius;
    let max_x = arena.origin.x + arena.size.width - ball.radius;
    let min_y = arena.origin.y + ball.radius;
    let max_y = arena.origin.y + arena.size.height - ball.radius;
    if ball.position.x < min_x {
        ball.position.x = min_x;
        ball.velocity.x = ball.velocity.x.abs();
    } else if ball.position.x > max_x {
        ball.position.x = max_x;
        ball.velocity.x = -ball.velocity.x.abs();
    }
    if ball.position.y < min_y {
        ball.position.y = min_y;
        ball.velocity.y = ball.velocity.y.abs();
    } else if ball.position.y > max_y {
        ball.position.y = max_y;
        ball.velocity.y = -ball.velocity.y.abs();
    }
}

fn resolve_pair_collision(first: &mut Ball, second: &mut Ball) {
    let delta = Vec2::new(
        second.position.x - first.position.x,
        second.position.y - first.position.y,
    );
    let min_distance = first.radius + second.radius;
    let distance_squared = delta.length_squared();
    if distance_squared > min_distance * min_distance {
        return;
    }
    let (normal_x, normal_y, distance) = if distance_squared <= f32::EPSILON {
        (1.0, 0.0, min_distance)
    } else {
        let distance = distance_squared.sqrt();
        (delta.x / distance, delta.y / distance, distance)
    };
    let overlap = (min_distance - distance).max(0.0);
    if overlap > 0.0 {
        let correction = overlap * 0.5 + 0.01;
        first.position.x -= normal_x * correction;
        first.position.y -= normal_y * correction;
        second.position.x += normal_x * correction;
        second.position.y += normal_y * correction;
    }
    let first_normal_velocity = first.velocity.x * normal_x + first.velocity.y * normal_y;
    let second_normal_velocity = second.velocity.x * normal_x + second.velocity.y * normal_y;
    let relative_velocity = second_normal_velocity - first_normal_velocity;
    if relative_velocity >= 0.0 {
        return;
    }
    let exchanged = second_normal_velocity - first_normal_velocity;
    first.velocity.x += normal_x * exchanged;
    first.velocity.y += normal_y * exchanged;
    second.velocity.x -= normal_x * exchanged;
    second.velocity.y -= normal_y * exchanged;
}

fn ball_color(index: usize) -> Color {
    const PALETTE: [Color; 8] = [
        Color::rgba(111, 221, 255, 255),
        Color::rgba(130, 255, 170, 255),
        Color::rgba(255, 208, 102, 255),
        Color::rgba(255, 154, 162, 255),
        Color::rgba(188, 148, 255, 255),
        Color::rgba(255, 131, 227, 255),
        Color::rgba(255, 244, 140, 255),
        Color::rgba(137, 180, 250, 255),
    ];
    PALETTE[index % PALETTE.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_initializes_requested_ball_count() {
        let world = BallWorld::new(BALL_COUNT);
        assert_eq!(world.balls.len(), BALL_COUNT);
    }

    #[test]
    fn wall_bounce_flips_velocity_inside_arena() {
        let arena = content_rect(Size::new(320.0, 240.0));
        let mut ball = Ball {
            position: Vec2::new(arena.origin.x + 2.0, arena.origin.y + 2.0),
            velocity: Vec2::new(-40.0, -30.0),
            radius: BALL_RADIUS,
            color: Color::WHITE,
        };
        bounce_against_bounds(&mut ball, arena);
        assert!(ball.velocity.x > 0.0);
        assert!(ball.velocity.y > 0.0);
    }

    #[test]
    fn pair_collision_separates_overlap_and_exchanges_direction() {
        let mut first = Ball {
            position: Vec2::new(100.0, 100.0),
            velocity: Vec2::new(40.0, 0.0),
            radius: BALL_RADIUS,
            color: Color::WHITE,
        };
        let mut second = Ball {
            position: Vec2::new(118.0, 100.0),
            velocity: Vec2::new(-20.0, 0.0),
            radius: BALL_RADIUS,
            color: Color::WHITE,
        };
        resolve_pair_collision(&mut first, &mut second);
        assert!(first.velocity.x <= -19.9);
        assert!(second.velocity.x >= 39.9);
    }

    #[test]
    fn hit_test_selects_expected_demo() {
        let viewport = Size::new(1200.0, 800.0);
        let rect = demo_button_rect(viewport, DemoKind::Compose);
        let point = Point::new(rect.origin.x + 10.0, rect.origin.y + 10.0);
        assert_eq!(
            hit_test_demo_button(point, viewport),
            Some(DemoKind::Compose)
        );
    }

    #[test]
    fn reordered_labels_rotate_stably() {
        assert_eq!(
            reordered_labels(1),
            vec!["Layer", "Text", "Backend", "Patch"]
        );
    }

    #[test]
    fn inactive_demo_stops_advancing_physics_world() {
        let mut app = AppState::new();
        let first_context = AppFrame {
            frame_index: 0,
            elapsed: Duration::from_millis(16),
            delta: Duration::from_millis(16),
            size: Size::new(1280.0, 800.0),
            platform: zeno_core::Platform::MacOs,
            backend: Backend::Impeller,
            last_report: None,
            pointer: zeno_runtime::PointerState::default(),
        };
        let initial_position = app.world.balls[0].position;
        let _ = <AppState as App>::render(&mut app, &first_context);
        let advanced_position = app.world.balls[0].position;
        assert_ne!(advanced_position, initial_position);

        app.active_demo = DemoKind::Compose;
        let second_context = AppFrame {
            frame_index: 1,
            elapsed: Duration::from_millis(32),
            delta: Duration::from_millis(16),
            size: Size::new(1280.0, 800.0),
            platform: zeno_core::Platform::MacOs,
            backend: Backend::Impeller,
            last_report: None,
            pointer: zeno_runtime::PointerState::default(),
        };
        let _ = <AppState as App>::render(&mut app, &second_context);
        assert_eq!(app.world.balls[0].position, advanced_position);
    }

    #[test]
    fn compositor_scene_contains_multiple_layers() {
        let scene = build_compositor_scene(
            &SystemTextSystem,
            Size::new(1280.0, 800.0),
            Backend::Impeller,
            60.0,
            1.2,
            DemoKind::Compositor,
            None,
        );
        assert!(scene.layers.len() >= 4);
    }
}

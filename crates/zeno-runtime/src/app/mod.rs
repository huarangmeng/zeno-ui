use std::time::Duration;

use zeno_core::{Backend, Platform, Point, Size};
use zeno_scene::FrameReport;
use zeno_ui::Node;

#[derive(Debug, Clone, PartialEq)]
pub struct PointerState {
    pub position: Option<Point>,
    pub pressed: bool,
    pub just_pressed: bool,
    pub just_released: bool,
}

impl Default for PointerState {
    fn default() -> Self {
        Self {
            position: None,
            pressed: false,
            just_pressed: false,
            just_released: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppFrame {
    pub frame_index: u64,
    pub elapsed: Duration,
    pub delta: Duration,
    pub size: Size,
    pub platform: Platform,
    pub backend: Backend,
    pub last_report: Option<FrameReport>,
    pub pointer: PointerState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppView {
    Compose(Node),
}

pub trait App {
    fn render(&mut self, frame: &AppFrame) -> AppView;

    fn animation_interval(&self, _frame: &AppFrame) -> Option<Duration> {
        None
    }
}

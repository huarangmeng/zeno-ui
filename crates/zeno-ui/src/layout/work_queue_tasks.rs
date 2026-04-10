use std::collections::VecDeque;

use zeno_core::{Point, Size};

/// 两阶段工作队列：intrinsic → placement
#[derive(Default)]
pub(crate) struct LayoutWorkQueue {
    tasks: VecDeque<LayoutTask>,
}

impl LayoutWorkQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, task: LayoutTask) {
        self.tasks.push_back(task);
    }

    pub fn pop(&mut self) -> Option<LayoutTask> {
        self.tasks.pop_back()
    }
}

#[derive(Debug, Clone)]
pub(crate) enum LayoutTask {
    Measure {
        index: usize,
        origin: Point,
        available: Size,
    },
    FinalizeContainer {
        index: usize,
        origin: Point,
        available: Size,
        child_index: usize,
    },
    FinalizeBox {
        index: usize,
        origin: Point,
        available: Size,
        child_indices: Vec<usize>,
    },
    ContinueStack {
        index: usize,
        origin: Point,
        available: Size,
        child_indices: Vec<usize>,
        next_child_offset: usize,
        used_main: f32,
    },
    ResumeStack {
        index: usize,
        origin: Point,
        available: Size,
        child_indices: Vec<usize>,
        measured_child_offset: usize,
        used_main_before_child: f32,
    },
}


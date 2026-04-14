use std::collections::VecDeque;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use crate::composite::CompositorSubmission;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorTask {
    Raster,
    Composite,
    Present,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledCompositorFrame {
    pub generation: u64,
    pub submission: CompositorSubmission,
    pub tasks: Vec<CompositorTask>,
    pub enqueued_frame_count: usize,
    pub stale_frame_count: usize,
    pub dropped_frame_count: usize,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CompositorScheduler {
    latest_generation: Option<u64>,
    pending_frames: VecDeque<(u64, CompositorSubmission)>,
    dropped_frame_count: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CompositorSchedulerStats {
    pub pending_frame_count: usize,
    pub dropped_frame_count: usize,
}

impl CompositorScheduler {
    const MAX_QUEUE_DEPTH: usize = 2;

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue_frame(&mut self, generation: u64, submission: CompositorSubmission) {
        self.latest_generation = Some(generation);
        self.pending_frames.push_back((generation, submission));
        while self.pending_frames.len() > Self::MAX_QUEUE_DEPTH {
            let _ = self.pending_frames.pop_front();
            self.dropped_frame_count += 1;
        }
    }

    pub fn take_latest(&mut self) -> Option<ScheduledCompositorFrame> {
        let enqueued_frame_count = self.pending_frames.len();
        let (generation, submission) = self.pending_frames.pop_back()?;
        let stale_frame_count = self.pending_frames.len();
        self.dropped_frame_count += stale_frame_count;
        self.pending_frames.clear();
        let mut tasks = Vec::new();
        if submission.raster_batch.tile_count() > 0 {
            tasks.push(CompositorTask::Raster);
        }
        if submission.composite_pass.layer_count() > 0 {
            tasks.push(CompositorTask::Composite);
        }
        tasks.push(CompositorTask::Present);
        Some(ScheduledCompositorFrame {
            generation,
            submission,
            tasks,
            enqueued_frame_count,
            stale_frame_count,
            dropped_frame_count: self.dropped_frame_count,
        })
    }

    #[must_use]
    pub fn schedule(
        &mut self,
        generation: u64,
        submission: CompositorSubmission,
    ) -> ScheduledCompositorFrame {
        self.enqueue_frame(generation, submission);
        self.take_latest()
            .expect("scheduled frame should be available after enqueue")
    }

    #[must_use]
    pub fn latest_generation(&self) -> Option<u64> {
        self.latest_generation
    }

    #[must_use]
    pub fn dropped_frame_count(&self) -> usize {
        self.dropped_frame_count
    }

    #[must_use]
    pub fn stats(&self) -> CompositorSchedulerStats {
        CompositorSchedulerStats {
            pending_frame_count: self.pending_frames.len(),
            dropped_frame_count: self.dropped_frame_count,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CompositorWorkerStats {
    pub submitted_frame_count: usize,
    pub processed_frame_count: usize,
    pub latest_generation: Option<u64>,
    pub dropped_frame_count: usize,
    pub worker_threaded: bool,
    pub worker_alive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositorWorkerOutput {
    pub scheduled: ScheduledCompositorFrame,
    pub worker_stats: CompositorWorkerStats,
    pub scheduler_stats: CompositorSchedulerStats,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CompositorServiceStats {
    pub submitted_frame_count: usize,
    pub processed_frame_count: usize,
    pub dropped_frame_count: usize,
    pub worker_threaded: bool,
    pub worker_alive: bool,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CompositorWorker {
    scheduler: CompositorScheduler,
    submitted_frame_count: usize,
    processed_frame_count: usize,
}

impl CompositorWorker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn submit_frame(
        &mut self,
        generation: u64,
        submission: CompositorSubmission,
    ) -> ScheduledCompositorFrame {
        self.submitted_frame_count += 1;
        let scheduled = self.scheduler.schedule(generation, submission);
        self.processed_frame_count += 1;
        scheduled
    }

    #[must_use]
    pub fn stats(&self) -> CompositorWorkerStats {
        CompositorWorkerStats {
            submitted_frame_count: self.submitted_frame_count,
            processed_frame_count: self.processed_frame_count,
            latest_generation: self.scheduler.latest_generation(),
            dropped_frame_count: self.scheduler.dropped_frame_count(),
            worker_threaded: false,
            worker_alive: true,
        }
    }

    #[must_use]
    pub fn scheduler_stats(&self) -> CompositorSchedulerStats {
        self.scheduler.stats()
    }
}

enum ThreadedWorkerCommand {
    Submit(u64, CompositorSubmission),
    Shutdown,
}

pub struct ThreadedCompositorWorker {
    command_tx: mpsc::Sender<ThreadedWorkerCommand>,
    result_rx: mpsc::Receiver<CompositorWorkerOutput>,
    join_handle: Option<JoinHandle<()>>,
    pub(crate) submitted_frame_count: usize,
}

impl ThreadedCompositorWorker {
    #[must_use]
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::channel::<ThreadedWorkerCommand>();
        let (result_tx, result_rx) = mpsc::channel::<CompositorWorkerOutput>();
        let join_handle = thread::spawn(move || {
            let mut worker = CompositorWorker::new();
            while let Ok(command) = command_rx.recv() {
                match command {
                    ThreadedWorkerCommand::Submit(generation, submission) => {
                        let scheduled = worker.submit_frame(generation, submission);
                        let output = CompositorWorkerOutput {
                            scheduled,
                            worker_stats: worker.stats(),
                            scheduler_stats: worker.scheduler_stats(),
                        };
                        if result_tx.send(output).is_err() {
                            break;
                        }
                    }
                    ThreadedWorkerCommand::Shutdown => break,
                }
            }
        });
        Self {
            command_tx,
            result_rx,
            join_handle: Some(join_handle),
            submitted_frame_count: 0,
        }
    }

    pub fn submit_frame(
        &mut self,
        generation: u64,
        submission: CompositorSubmission,
    ) -> Result<CompositorWorkerOutput, String> {
        self.submitted_frame_count += 1;
        self.command_tx
            .send(ThreadedWorkerCommand::Submit(generation, submission))
            .map_err(|error| error.to_string())?;
        let mut output = self.result_rx.recv().map_err(|error| error.to_string())?;
        output.worker_stats.submitted_frame_count = self.submitted_frame_count;
        output.worker_stats.worker_threaded = true;
        output.worker_stats.worker_alive = self.join_handle.is_some();
        Ok(output)
    }
}

impl Drop for ThreadedCompositorWorker {
    fn drop(&mut self) {
        let _ = self.command_tx.send(ThreadedWorkerCommand::Shutdown);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

pub struct CompositorService {
    worker: ThreadedCompositorWorker,
    last_stats: CompositorServiceStats,
}

impl CompositorService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            worker: ThreadedCompositorWorker::new(),
            last_stats: CompositorServiceStats::default(),
        }
    }

    pub fn submit_frame(
        &mut self,
        generation: u64,
        submission: CompositorSubmission,
    ) -> Result<CompositorWorkerOutput, String> {
        let output = self.worker.submit_frame(generation, submission)?;
        self.last_stats = CompositorServiceStats {
            submitted_frame_count: output.worker_stats.submitted_frame_count,
            processed_frame_count: output.worker_stats.processed_frame_count,
            dropped_frame_count: output.worker_stats.dropped_frame_count,
            worker_threaded: output.worker_stats.worker_threaded,
            worker_alive: output.worker_stats.worker_alive,
        };
        Ok(output)
    }

    #[must_use]
    pub fn stats(&self) -> CompositorServiceStats {
        self.last_stats
    }
}

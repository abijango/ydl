//! Terminal [`EventSink`] that renders download events as `indicatif` bars.
//!
//! This owns the presentation concerns that used to live in `download::run`:
//! the pool of reusable worker-bar "slots" and the overall counter. Core emits
//! per-item events keyed by a global `id`; this maps them back onto a fixed set
//! of bars so the terminal shows only `jobs` rows, reused — exactly as before.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use indicatif::{MultiProgress, ProgressBar};
use ydl::event::{DownloadEvent, EventSink};
use ydl::progress;

#[derive(Clone)]
pub struct CliSink {
    inner: Arc<Inner>,
}

struct Inner {
    mp: MultiProgress,
    state: Mutex<State>,
}

struct State {
    overall: Option<ProgressBar>,
    free_slots: Vec<usize>,
    bars: HashMap<u64, Worker>,
}

struct Worker {
    slot: usize,
    bar: ProgressBar,
    url: String,
}

impl CliSink {
    pub fn new(jobs: usize) -> Self {
        let jobs = jobs.max(1);
        Self {
            inner: Arc::new(Inner {
                mp: MultiProgress::new(),
                state: Mutex::new(State {
                    overall: None,
                    free_slots: (0..jobs).collect(),
                    bars: HashMap::new(),
                }),
            }),
        }
    }

    /// Finish the overall bar (if any) once the run completes.
    pub fn finish(&self, failures: usize) {
        let state = self.inner.state.lock().expect("cli sink state");
        if let Some(pb) = &state.overall {
            pb.finish_with_message(if failures == 0 {
                "all done".to_string()
            } else {
                format!("done with {failures} failure(s)")
            });
        }
    }
}

impl EventSink for CliSink {
    fn emit(&self, event: DownloadEvent) {
        let mut state = self.inner.state.lock().expect("cli sink state");
        match event {
            DownloadEvent::Expanded { total, .. } => {
                if total > 1 && state.overall.is_none() {
                    let pb = progress::make_overall_bar(&self.inner.mp, total as u64);
                    pb.set_message("downloading…");
                    state.overall = Some(pb);
                }
            }
            DownloadEvent::Started { id, url } => {
                let slot = state.free_slots.pop().unwrap_or(0);
                let bar = progress::make_worker_bar(&self.inner.mp, slot);
                state.bars.insert(id, Worker { slot, bar, url });
            }
            DownloadEvent::Progress {
                id,
                downloaded,
                total,
                title,
                ..
            } => {
                if let Some(w) = state.bars.get(&id) {
                    if total > 0 {
                        w.bar.set_length(total);
                    }
                    w.bar.set_position(downloaded);
                    if let Some(t) = title {
                        w.bar.set_message(t);
                    }
                }
            }
            DownloadEvent::Status { id, message } => {
                if let Some(w) = state.bars.get(&id) {
                    w.bar.set_message(message);
                }
            }
            DownloadEvent::Completed { id, skipped, .. } => {
                if let Some(w) = state.bars.remove(&id) {
                    if skipped {
                        w.bar
                            .finish_with_message(format!("⊘ {} (skipped)", w.url));
                    } else {
                        w.bar.finish_with_message(format!("✓ {}", w.url));
                    }
                    state.free_slots.push(w.slot);
                }
                if let Some(pb) = &state.overall {
                    pb.inc(1);
                }
            }
            DownloadEvent::Failed { id, error } => {
                if let Some(w) = state.bars.remove(&id) {
                    w.bar
                        .finish_with_message(format!("✗ {} — {error}", w.url));
                    state.free_slots.push(w.slot);
                }
                if let Some(pb) = &state.overall {
                    pb.inc(1);
                }
            }
        }
    }
}

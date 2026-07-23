//! [`EventSink`] that forwards download events to the webview.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use ydl::event::{DownloadEvent, EventSink};

const PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(100);

struct ThrottleState {
    last_emit: Instant,
    last_downloaded: u64,
    last_total: u64,
    pending: Option<DownloadEvent>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GuiEvent {
    run_id: u64,
    #[serde(flatten)]
    event: DownloadEvent,
}

/// Bridges the core download pipeline to the frontend. Every [`DownloadEvent`]
/// is emitted on the global `ydl://event` channel; the frontend listens once and
/// routes by the event's `id`.
///
/// Progress events are throttled to ~10 Hz per download id (with 1%-point
/// coalescing). Each payload includes `runId` so the UI can ignore stale runs.
#[derive(Clone)]
pub struct TauriSink {
    app: AppHandle,
    run_id: u64,
    throttle: Arc<Mutex<HashMap<u64, ThrottleState>>>,
}

impl TauriSink {
    pub fn new(app: AppHandle, run_id: u64) -> Self {
        Self {
            app,
            run_id,
            throttle: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn emit_raw(&self, event: DownloadEvent) {
        let _ = self.app.emit(
            "ydl://event",
            GuiEvent {
                run_id: self.run_id,
                event,
            },
        );
    }

    fn pct(downloaded: u64, total: u64) -> u64 {
        if total == 0 {
            0
        } else {
            (downloaded.saturating_mul(100)) / total
        }
    }

    fn should_emit_progress(state: &ThrottleState, downloaded: u64, total: u64, now: Instant) -> bool {
        if now.duration_since(state.last_emit) >= PROGRESS_MIN_INTERVAL {
            return true;
        }
        Self::pct(downloaded, total) != Self::pct(state.last_downloaded, state.last_total)
    }

    fn take_pending(&self, id: u64) -> Option<DownloadEvent> {
        let mut map = self.throttle.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(&id).and_then(|s| s.pending)
    }
}

impl EventSink for TauriSink {
    fn emit(&self, event: DownloadEvent) {
        let terminal_id = match &event {
            DownloadEvent::Completed { id, .. } | DownloadEvent::Failed { id, .. } => Some(*id),
            _ => None,
        };

        if let Some(id) = terminal_id {
            if let Some(pending) = self.take_pending(id) {
                self.emit_raw(pending);
            }
            self.emit_raw(event);
            return;
        }

        match event {
            DownloadEvent::Progress {
                id,
                downloaded,
                total,
                speed,
                eta,
                title,
            } => {
                let now = Instant::now();
                let event = DownloadEvent::Progress {
                    id,
                    downloaded,
                    total,
                    speed,
                    eta,
                    title,
                };
                let mut map = self.throttle.lock().unwrap_or_else(|e| e.into_inner());
                let state = map.entry(id).or_insert_with(|| ThrottleState {
                    last_emit: now
                        .checked_sub(PROGRESS_MIN_INTERVAL)
                        .unwrap_or(now),
                    last_downloaded: 0,
                    last_total: 0,
                    pending: None,
                });
                if Self::should_emit_progress(state, downloaded, total, now) {
                    state.last_emit = now;
                    state.last_downloaded = downloaded;
                    state.last_total = total;
                    state.pending = None;
                    drop(map);
                    self.emit_raw(event);
                } else {
                    state.pending = Some(event);
                }
            }
            other => self.emit_raw(other),
        }
    }
}

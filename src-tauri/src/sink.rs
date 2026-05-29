//! [`EventSink`] that forwards download events to the webview.

use tauri::{AppHandle, Emitter};
use ydl::event::{DownloadEvent, EventSink};

/// Bridges the core download pipeline to the frontend. Every [`DownloadEvent`]
/// is emitted on the global `ydl://event` channel; the frontend listens once and
/// routes by the event's `id`.
#[derive(Clone)]
pub struct TauriSink {
    app: AppHandle,
}

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl EventSink for TauriSink {
    fn emit(&self, event: DownloadEvent) {
        // Best-effort: if the window is gone, a download task shouldn't panic.
        let _ = self.app.emit("ydl://event", event);
    }
}

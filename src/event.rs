//! The presentation seam between the download pipeline and any UI.
//!
//! [`download::run_with_sink`](crate::download::run_with_sink) emits a stream of
//! [`DownloadEvent`]s through an [`EventSink`]. The CLI implements a sink that
//! drives `indicatif` progress bars; the Tauri app implements one that forwards
//! events to the webview. Neither concern leaks into the core pipeline.

use serde::Serialize;

/// A single update about the lifecycle of a download run.
///
/// Serialized with an internally-tagged `type` field so the frontend can
/// `switch` on it directly (e.g. `{ "type": "progress", "id": 0, ... }`).
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DownloadEvent {
    /// The work-list is known. For playlists/channels this fires after expansion.
    Expanded {
        total: usize,
        #[serde(rename = "playlistTitle")]
        playlist_title: Option<String>,
    },
    /// A worker picked up `url` and assigned it the stable `id` used by every
    /// subsequent event for this item.
    Started { id: u64, url: String },
    /// A progress tick. `total == 0` means the size is not yet known.
    Progress {
        id: u64,
        downloaded: u64,
        total: u64,
        /// Bytes per second as reported by yt-dlp (0 if unknown).
        speed: f64,
        /// Seconds remaining as reported by yt-dlp (0 if unknown).
        eta: u64,
        title: Option<String>,
    },
    /// A human-readable status transition, e.g. `"merging…"` or
    /// `"skipped (in archive)"`.
    Status { id: u64, message: String },
    /// The item finished successfully (or was skipped).
    Completed {
        id: u64,
        title: Option<String>,
        path: Option<String>,
        bytes: u64,
        skipped: bool,
    },
    /// The item failed; `error` is a one-line message.
    Failed { id: u64, error: String },
}

/// A consumer of [`DownloadEvent`]s.
///
/// Sinks are cloned into each download task, so implementations must be cheap to
/// clone and use interior mutability (e.g. wrap shared state in `Arc<Mutex<_>>`).
/// `emit` is synchronous and must not block for long.
pub trait EventSink: Send + Sync + Clone + 'static {
    fn emit(&self, event: DownloadEvent);
}

/// A sink that discards every event. Useful for tests and dry runs.
#[derive(Clone, Default)]
pub struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _event: DownloadEvent) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_event_serializes_camel_case_type_tag() {
        let e = DownloadEvent::Progress {
            id: 1,
            downloaded: 10,
            total: 100,
            speed: 1.5,
            eta: 9,
            title: Some("t".into()),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["type"], "progress");
        assert_eq!(v["id"], 1);
        assert_eq!(v["downloaded"], 10);
        assert!(v.get("downloadedBytes").is_none());
    }

    #[test]
    fn expanded_event_uses_playlist_title_camel_case() {
        let e = DownloadEvent::Expanded {
            total: 3,
            playlist_title: Some("Mix".into()),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["type"], "expanded");
        assert_eq!(v["playlistTitle"], "Mix");
        assert_eq!(v["total"], 3);
    }
}

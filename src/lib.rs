//! Core library for `ydl`: configuration, dependency management, and the
//! yt-dlp/ffmpeg download orchestration.
//!
//! Everything here is UI-agnostic. Presentation is driven through the
//! [`event::EventSink`] abstraction, so both the terminal CLI and the Tauri
//! desktop app can consume the same download pipeline as structured data.

pub mod archive;
pub mod cli;
pub mod config;
pub mod deps;
pub mod download;
pub mod error;
pub mod event;
pub mod progress;
pub mod summary;
pub mod ytdlp;

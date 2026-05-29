//! Tauri commands — thin wrappers that reuse the `ydl` core library.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use ydl::cli::DownloadOpts;
use ydl::config::{self, Config};
use ydl::deps::{self, Tool};
use ydl::download::{self, Mode};

use crate::sink::TauriSink;

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

/// Load the resolved config (creating a default file on first run).
#[tauri::command]
pub fn get_config() -> Result<Config, String> {
    config::load_or_init().map_err(err)
}

/// Persist edited settings back to the standard config path.
#[tauri::command]
pub fn save_config(config: Config) -> Result<(), String> {
    config::save(&config).map_err(err)
}

/// Classify a pasted URL so the UI can show the right mode badge.
/// Returns `"single"` or `"playlist"` (channels expand like playlists).
#[tauri::command]
pub fn classify_url(url: String) -> String {
    match download::classify_url(&url) {
        Mode::Single => "single",
        Mode::Playlist => "playlist",
        Mode::Batch => "batch",
    }
    .to_string()
}

/// Status of a dependency, for the header indicator and Settings panel.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepInfo {
    pub name: String,
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    /// True when the resolved binary is ydl's own managed copy (so it can be
    /// updated in-app); false for a system/PATH binary like Homebrew ffmpeg.
    pub managed: bool,
}

fn tool_from_name(name: &str) -> Option<Tool> {
    match name {
        "yt-dlp" => Some(Tool::YtDlp),
        "ffmpeg" => Some(Tool::Ffmpeg),
        _ => None,
    }
}

#[tauri::command]
pub async fn deps_status() -> Result<Vec<DepInfo>, String> {
    let cfg = config::load_or_init().map_err(err)?;
    let manifest = deps::load_manifest().await.unwrap_or_default();
    let bin_dir = config::bin_dir().ok();
    let mut out = Vec::new();
    for tool in [Tool::YtDlp, Tool::Ffmpeg] {
        let path = deps::resolve(&cfg, tool).ok().flatten();
        let managed = match (&path, &bin_dir) {
            (Some(p), Some(b)) => p.starts_with(b),
            _ => false,
        };
        let version = match tool {
            Tool::YtDlp => manifest.yt_dlp.as_ref(),
            Tool::Ffmpeg => manifest.ffmpeg.as_ref(),
        }
        .map(|v| v.version.clone());
        out.push(DepInfo {
            name: tool.label().to_string(),
            installed: path.is_some(),
            path: path.map(|p| p.display().to_string()),
            version,
            managed,
        });
    }
    Ok(out)
}

/// Install any missing managed binaries (yt-dlp, ffmpeg).
#[tauri::command]
pub async fn install_deps() -> Result<(), String> {
    let cfg = config::load_or_init().map_err(err)?;
    for tool in [Tool::YtDlp, Tool::Ffmpeg] {
        if deps::resolve(&cfg, tool).ok().flatten().is_none() {
            deps::install(tool).await.map_err(err)?;
        }
    }
    Ok(())
}

/// Re-download the latest managed copy of a single dependency.
#[tauri::command]
pub async fn update_dep(name: String) -> Result<(), String> {
    let tool = tool_from_name(&name).ok_or_else(|| format!("unknown dependency: {name}"))?;
    deps::install(tool).await.map_err(err)
}

/// Kick off a download run in the background. `urls` may be a multi-line string
/// (one URL per line → batch). Progress streams over `ydl://event`; the run ends
/// with either `ydl://summary` or `ydl://error`.
#[tauri::command]
pub async fn start_download(app: AppHandle, urls: String, audio_only: bool) -> Result<(), String> {
    let urls: Vec<String> = urls
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect();
    if urls.is_empty() {
        return Err("no URL provided".into());
    }
    let mode = if urls.len() > 1 {
        Mode::Batch
    } else {
        download::classify_url(&urls[0])
    };

    let mut cfg = config::load_or_init().map_err(err)?;
    // Non-interactive: the GUI auto-installs deps and never prompts.
    let opts = DownloadOpts {
        audio_only,
        yes: true,
        ..Default::default()
    };
    config::merge_opts(&mut cfg, &opts);

    let sink = TauriSink::new(app.clone());
    tauri::async_runtime::spawn(async move {
        match download::run_with_sink(&cfg, &opts, urls, mode, &sink).await {
            Ok(summary) => {
                let _ = app.emit("ydl://summary", summary.to_dto());
            }
            Err(e) => {
                let _ = app.emit("ydl://error", format!("{e:#}"));
            }
        }
    });
    Ok(())
}

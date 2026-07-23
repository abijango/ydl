//! Tauri commands — thin wrappers that reuse the `ydl` core library.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;
use ydl::DownloadOpts;
use ydl::config::{self, Config};
use ydl::deps::{self, Tool};
use ydl::download::{self, Mode};

use crate::sink::TauriSink;

/// Structured IPC error returned from Tauri commands (WP20).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl std::fmt::Display) -> Self {
        Self {
            code: code.into(),
            message: short_err(message),
        }
    }
}

impl From<CommandError> for String {
    fn from(e: CommandError) -> Self {
        e.message
    }
}

fn err(e: impl std::fmt::Display) -> String {
    short_err(e)
}

fn short_err(e: impl std::fmt::Display) -> String {
    let full = format!("{e:#}");
    full.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(&full)
        .trim()
        .to_string()
}

fn err_code(code: &str, e: impl std::fmt::Display) -> String {
    let msg = short_err(e);
    serde_json::to_string(&CommandError {
        code: code.into(),
        message: msg.clone(),
    })
    .unwrap_or(msg)
}

pub struct AppState {
    pub run_id: AtomicU64,
    pub busy: Arc<AtomicBool>,
    pub cancel: Mutex<Option<CancellationToken>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            run_id: AtomicU64::new(0),
            busy: Arc::new(AtomicBool::new(false)),
            cancel: Mutex::new(None),
        }
    }
}

const EXTRA_ARGS_DENY: &[&str] = &[
    "--exec",
    "--exec-before-download",
    "--exec-after-download",
    "--load-info-json",
    "--write-pages",
];

pub fn validate_config(cfg: &mut Config) -> Result<(), String> {
    if cfg.defaults.quality.trim().is_empty() {
        return Err("quality must not be empty".into());
    }
    if cfg.parallel.jobs == 0 {
        cfg.parallel.jobs = 1;
    }
    if cfg.parallel.jobs > 32 {
        cfg.parallel.jobs = 32;
    }
    for arg in &cfg.ytdlp.extra_args {
        let a = arg.trim();
        for deny in EXTRA_ARGS_DENY {
            if a == *deny || a.starts_with(&format!("{deny}=")) {
                return Err(format!("extra_args must not include {deny}"));
            }
        }
    }
    if cfg.defaults.output_dir.as_os_str().is_empty() {
        return Err("output_dir must not be empty".into());
    }
    Ok(())
}

/// The app's own version (CalVer, baked in at build time). Shown in the UI.
#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Reveal a path in the OS file manager. Directories open directly; files are
/// revealed (highlighted) in their containing folder. Done in Rust so it
/// bypasses the JS opener plugin's path-allowlist scope.
#[tauri::command]
pub fn reveal_path(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let is_dir = std::path::Path::new(&path).is_dir();
    if is_dir {
        app.opener().open_path(path, None::<&str>).map_err(err)
    } else {
        app.opener().reveal_item_in_dir(path).map_err(err)
    }
}

// ── Download history ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct HistoryEntry {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub path: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub bytes: u64,
    pub ts: i64, // epoch milliseconds
}

fn history_path() -> Result<PathBuf, String> {
    Ok(config::data_dir().map_err(err)?.join("history.json"))
}

fn read_history_file() -> Vec<HistoryEntry> {
    history_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_history_file(items: &[HistoryEntry]) -> Result<(), String> {
    let p = history_path()?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    std::fs::write(&p, serde_json::to_string_pretty(items).map_err(err)?).map_err(err)
}

#[tauri::command]
pub fn get_history() -> Vec<HistoryEntry> {
    read_history_file()
}

#[tauri::command]
pub fn add_history(entry: HistoryEntry) -> Result<(), String> {
    let mut items = read_history_file();
    items.retain(|e| e.id != entry.id); // de-dup by id
    items.insert(0, entry); // newest first
    items.truncate(1000); // keep history bounded
    write_history_file(&items)
}

#[tauri::command]
pub fn remove_history(id: String) -> Result<(), String> {
    let mut items = read_history_file();
    items.retain(|e| e.id != id);
    write_history_file(&items)
}

#[tauri::command]
pub fn clear_history() -> Result<(), String> {
    write_history_file(&[])
}

/// Load the resolved config (creating a default file on first run).
#[tauri::command]
pub fn get_config() -> Result<Config, String> {
    config::load_or_init().map_err(err)
}

#[tauri::command]
pub fn save_config(mut config: Config) -> Result<(), String> {
    validate_config(&mut config)?;
    config::save(&config).map_err(err)
}

#[tauri::command]
pub fn resolve_output_dir() -> Result<String, String> {
    let cfg = config::load_or_init().map_err(err)?;
    Ok(download::absolute_dir(&cfg.defaults.output_dir)
        .display()
        .to_string())
}

#[tauri::command]
pub fn classify_url(url: String) -> String {
    match download::classify_url(&url) {
        Mode::Single => "single",
        Mode::Playlist => "playlist",
        Mode::Batch => "batch",
    }
    .to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepInfo {
    pub name: String,
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub managed: bool,
    pub install_hint: Option<String>,
}

fn tool_from_name(name: &str) -> Option<Tool> {
    match name {
        "yt-dlp" => Some(Tool::YtDlp),
        "ffmpeg" => Some(Tool::Ffmpeg),
        _ => None,
    }
}

fn install_hint(tool: Tool, installed: bool) -> Option<String> {
    if installed {
        return None;
    }
    if tool == Tool::Ffmpeg && cfg!(target_os = "macos") {
        Some("Install ffmpeg with: brew install ffmpeg".into())
    } else {
        None
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
        .map(|v| deps::short_version(tool, &v.version));
        let installed = path.is_some();
        out.push(DepInfo {
            name: tool.label().to_string(),
            installed,
            path: path.map(|p| p.display().to_string()),
            version,
            managed,
            install_hint: install_hint(tool, installed),
        });
    }
    Ok(out)
}

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

#[tauri::command]
pub async fn update_dep(name: String) -> Result<(), String> {
    let tool = tool_from_name(&name).ok_or_else(|| format!("unknown dependency: {name}"))?;
    deps::install(tool).await.map_err(err)
}

#[tauri::command]
pub async fn open_download_path(app: AppHandle, path: String) -> Result<(), String> {
    let cfg = config::load_or_init().map_err(err)?;
    let output = download::absolute_dir(&cfg.defaults.output_dir);
    let candidate = PathBuf::from(&path);
    let candidate = if candidate.is_absolute() {
        candidate
    } else {
        output.join(&candidate)
    };
    let cand = soft_canonicalize(&candidate)?;
    let out = soft_canonicalize(&output).unwrap_or(output);
    if !path_under(&cand, &out) {
        return Err(format!(
            "path is outside the configured output directory ({})",
            out.display()
        ));
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(cand.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn soft_canonicalize(p: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(p).or_else(|_| {
        if p.is_absolute() {
            Ok(p.to_path_buf())
        } else {
            Ok(std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(p))
        }
    })
}

fn path_under(child: &Path, parent: &Path) -> bool {
    child.starts_with(parent)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunErrorPayload {
    run_id: u64,
    message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SummaryPayload {
    run_id: u64,
    #[serde(flatten)]
    summary: ydl::summary::SummaryDto,
}

#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    state: State<'_, AppState>,
    urls: String,
    audio_only: bool,
) -> Result<u64, String> {
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err(err_code("busy", "download already in progress"));
    }

    let urls: Vec<String> = urls
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect();
    if urls.is_empty() {
        state.busy.store(false, Ordering::SeqCst);
        return Err(err_code("no_url", "no URL provided"));
    }
    let mode = if urls.len() > 1 {
        Mode::Batch
    } else {
        download::classify_url(&urls[0])
    };

    let mut cfg = match config::load_or_init() {
        Ok(c) => c,
        Err(e) => {
            state.busy.store(false, Ordering::SeqCst);
            return Err(err(e));
        }
    };
    let opts = DownloadOpts {
        audio_only,
        yes: true,
        ..Default::default()
    };
    config::merge_opts(&mut cfg, &opts);

    let run_id = state.run_id.fetch_add(1, Ordering::SeqCst) + 1;
    let cancel = CancellationToken::new();
    {
        let mut slot = state.cancel.lock().map_err(|e| e.to_string())?;
        *slot = Some(cancel.clone());
    }

    let sink = TauriSink::new(app.clone(), run_id);
    let busy = Arc::clone(&state.busy);

    tauri::async_runtime::spawn(async move {
        let result =
            download::run_with_sink_cancel(&cfg, &opts, urls, mode, &sink, Some(cancel)).await;
        match result {
            Ok(summary) => {
                let _ = app.emit(
                    "ydl://summary",
                    SummaryPayload {
                        run_id,
                        summary: summary.to_dto(),
                    },
                );
            }
            Err(e) => {
                let _ = app.emit(
                    "ydl://error",
                    RunErrorPayload {
                        run_id,
                        message: short_err(e),
                    },
                );
            }
        }
        busy.store(false, Ordering::SeqCst);
    });

    Ok(run_id)
}

#[tauri::command]
pub fn cancel_download(state: State<'_, AppState>) -> Result<(), String> {
    let slot = state.cancel.lock().map_err(|e| e.to_string())?;
    if let Some(token) = slot.as_ref() {
        token.cancel();
    }
    Ok(())
}

#[tauri::command]
pub fn clear_busy(state: State<'_, AppState>) -> Result<(), String> {
    state.busy.store(false, Ordering::SeqCst);
    if let Ok(mut slot) = state.cancel.lock() {
        *slot = None;
    }
    Ok(())
}

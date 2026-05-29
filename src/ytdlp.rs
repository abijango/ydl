use crate::archive;
use crate::config::Config;
use crate::error::{bail, Context, Result};
use crate::event::{DownloadEvent, EventSink};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

const PROGRESS_PREFIX: &str = "ydl-progress:";
const FINAL_PATH_PREFIX: &str = "ydl-final:";
const PLAYLIST_TITLE_PREFIX: &str = "ydl-pl-title:";
const PLAYLIST_ID_PREFIX: &str = "ydl-pl-id:";

const PROGRESS_TEMPLATE: &str = concat!(
    "ydl-progress:",
    r#"{"status":"%(progress.status)s","#,
    r#""downloaded":%(progress.downloaded_bytes,0)d,"#,
    r#""total":%(progress.total_bytes,progress.total_bytes_estimate,0)d,"#,
    r#""speed":%(progress.speed,0)d,"#,
    r#""eta":%(progress.eta,0)d,"#,
    r#""title":%(info.title)j}"#
);

#[derive(Debug, Default, Clone)]
pub struct DownloadOutcome {
    pub title: Option<String>,
    pub path: Option<PathBuf>,
    pub bytes: u64,
    pub skipped: bool,
}

#[derive(Debug, Deserialize)]
struct ProgressTick {
    status: String,
    #[serde(default)]
    downloaded: u64,
    #[serde(default)]
    total: u64,
    #[serde(default)]
    speed: f64,
    #[serde(default)]
    eta: u64,
    #[serde(default)]
    title: String,
}

pub struct DownloadCtx<'a> {
    pub cfg: &'a Config,
    pub ffmpeg_path: Option<&'a Path>,
    pub ytdlp_path: &'a Path,
}

/// Convert "{upload_date}-{title}.{ext}" into yt-dlp's "%(upload_date)s-%(title)s.%(ext)s".
/// If a token already uses `%(name)s` syntax it is passed through unchanged.
pub fn translate_template(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut name = String::new();
            let mut closed = false;
            while let Some(&nc) = chars.peek() {
                if nc == '}' {
                    chars.next();
                    closed = true;
                    break;
                }
                name.push(nc);
                chars.next();
            }
            if closed && !name.is_empty() {
                out.push_str(&format!("%({name})s"));
            } else {
                out.push('{');
                out.push_str(&name);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Build the output template path: <output_dir>/<translated filename>.
fn output_template(cfg: &Config) -> String {
    let dir = cfg.defaults.output_dir.to_string_lossy().into_owned();
    let name = translate_template(&cfg.defaults.filename_template);
    if dir.is_empty() || dir == "." {
        name
    } else {
        format!(
            "{}{}{}",
            dir.trim_end_matches(['/', '\\']),
            std::path::MAIN_SEPARATOR,
            name
        )
    }
}

/// Expand a playlist/channel URL into individual video URLs, and capture a
/// human-readable playlist title if available.
pub async fn expand_playlist(
    ctx: &DownloadCtx<'_>,
    url: &str,
) -> Result<(Option<String>, Vec<String>)> {
    let title_tpl = format!("{PLAYLIST_TITLE_PREFIX}%(playlist_title,channel,uploader)s");
    let id_tpl = format!("{PLAYLIST_ID_PREFIX}%(url,webpage_url,id)s");
    let out = Command::new(ctx.ytdlp_path)
        .arg("--flat-playlist")
        .arg("--print")
        .arg(&title_tpl)
        .arg("--print")
        .arg(&id_tpl)
        .arg("--no-warnings")
        .arg("--no-colors")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("spawn yt-dlp --flat-playlist")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!("yt-dlp --flat-playlist failed: {}", stderr.trim());
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut title: Option<String> = None;
    let mut ids: Vec<String> = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(t) = line.strip_prefix(PLAYLIST_TITLE_PREFIX) {
            if title.is_none() && !t.is_empty() && t != "NA" {
                title = Some(t.to_string());
            }
        } else if let Some(id) = line.strip_prefix(PLAYLIST_ID_PREFIX) {
            if !id.is_empty() {
                ids.push(id.to_string());
            }
        }
    }
    Ok((title, ids))
}

pub fn build_args(ctx: &DownloadCtx<'_>, url: &str) -> Vec<String> {
    let cfg = ctx.cfg;
    let mut args: Vec<String> = Vec::new();

    args.push("-o".into());
    args.push(output_template(cfg));

    if cfg.defaults.audio_only {
        args.push("-x".into());
        args.push("--audio-format".into());
        args.push("m4a".into());
    } else {
        args.push("-f".into());
        args.push(cfg.defaults.quality.clone());
        args.push("--merge-output-format".into());
        args.push(cfg.defaults.merge_format.clone());
    }

    if let Some(arch) = archive::path_for(cfg, &cfg.defaults.output_dir) {
        if let Some(parent) = arch.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        args.push("--download-archive".into());
        args.push(arch.to_string_lossy().into_owned());
    }

    if let Some(ff) = ctx.ffmpeg_path {
        args.push("--ffmpeg-location".into());
        args.push(ff.to_string_lossy().into_owned());
    }

    args.push("--newline".into());
    args.push("--progress".into());
    args.push("--no-colors".into());
    args.push("--no-warnings".into());
    args.push("--progress-template".into());
    args.push(PROGRESS_TEMPLATE.into());
    args.push("--print".into());
    args.push(format!("after_move:{FINAL_PATH_PREFIX}%(filepath)s"));

    args.extend(cfg.ytdlp.extra_args.iter().cloned());

    args.push(url.to_string());
    args
}

/// Download a single URL, emitting progress events for `id` through `sink`.
pub async fn download<S: EventSink>(
    ctx: &DownloadCtx<'_>,
    url: &str,
    id: u64,
    sink: &S,
) -> Result<DownloadOutcome> {
    let args = build_args(ctx, url);
    if let Some(d) = ctx.cfg.defaults.output_dir.as_path().parent() {
        let _ = tokio::fs::create_dir_all(d).await;
    }
    let _ = tokio::fs::create_dir_all(&ctx.cfg.defaults.output_dir).await;

    sink.emit(DownloadEvent::Status {
        id,
        message: "starting…".into(),
    });
    let mut child = Command::new(ctx.ytdlp_path)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("spawn {}", ctx.ytdlp_path.display()))?;

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    let sink_clone = sink.clone();
    let stdout_task = tokio::spawn(async move {
        let mut outcome = DownloadOutcome::default();
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            handle_stdout_line(&line, id, &sink_clone, &mut outcome);
        }
        outcome
    });

    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut tail: Vec<String> = Vec::new();
        while let Ok(Some(line)) = reader.next_line().await {
            tracing::debug!(target: "yt-dlp", "{}", line);
            if tail.len() >= 20 {
                tail.remove(0);
            }
            tail.push(line);
        }
        tail
    });

    let mut outcome = stdout_task.await.unwrap_or_default();
    let stderr_tail = stderr_task.await.unwrap_or_default();
    let status = child.wait().await.context("wait yt-dlp")?;
    if !status.success() {
        let joined = stderr_tail.join("\n");
        bail!(
            "yt-dlp exited with status {}: {}",
            status.code().unwrap_or(-1),
            joined.trim()
        );
    }

    if !outcome.skipped {
        if let Some(p) = &outcome.path {
            if let Ok(meta) = tokio::fs::metadata(p).await {
                outcome.bytes = meta.len();
            }
        }
    }

    Ok(outcome)
}

fn handle_stdout_line<S: EventSink>(
    line: &str,
    id: u64,
    sink: &S,
    outcome: &mut DownloadOutcome,
) {
    if let Some(rest) = line.strip_prefix(PROGRESS_PREFIX) {
        match serde_json::from_str::<ProgressTick>(rest) {
            Ok(tick) => apply_tick(&tick, id, sink, outcome),
            Err(e) => tracing::trace!("progress parse error: {e} | line: {rest}"),
        }
        return;
    }
    if let Some(p) = line.strip_prefix(FINAL_PATH_PREFIX) {
        let p = p.trim();
        if !p.is_empty() && p != "NA" {
            outcome.path = Some(PathBuf::from(p));
        }
        return;
    }
    if line.contains("[download]") && line.contains("has already been recorded") {
        outcome.skipped = true;
        sink.emit(DownloadEvent::Status {
            id,
            message: "skipped (in archive)".into(),
        });
        return;
    }
    if line.contains("[Merger]") {
        sink.emit(DownloadEvent::Status {
            id,
            message: "merging…".into(),
        });
    }
    tracing::debug!(target: "yt-dlp", "{}", line);
}

fn apply_tick<S: EventSink>(t: &ProgressTick, id: u64, sink: &S, outcome: &mut DownloadOutcome) {
    let title = if t.title.is_empty() {
        None
    } else {
        outcome.title = Some(t.title.clone());
        Some(t.title.clone())
    };
    match t.status.as_str() {
        "downloading" => sink.emit(DownloadEvent::Progress {
            id,
            downloaded: t.downloaded,
            total: t.total,
            speed: t.speed,
            eta: t.eta,
            title,
        }),
        "finished" => sink.emit(DownloadEvent::Progress {
            id,
            downloaded: if t.total > 0 { t.total } else { t.downloaded },
            total: t.total,
            speed: 0.0,
            eta: 0,
            title,
        }),
        "error" => sink.emit(DownloadEvent::Status {
            id,
            message: "error".into(),
        }),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_braces_to_yt_dlp_syntax() {
        assert_eq!(
            translate_template("{upload_date}-{title}.{ext}"),
            "%(upload_date)s-%(title)s.%(ext)s"
        );
    }

    #[test]
    fn passes_through_native_syntax() {
        assert_eq!(translate_template("%(title)s.%(ext)s"), "%(title)s.%(ext)s");
    }
}

use crate::archive;
use crate::config::Config;
use crate::error::{bail, Context, Result};
use indicatif::ProgressBar;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

const PROGRESS_PREFIX: &str = "ydl-progress:";

const PROGRESS_TEMPLATE: &str = concat!(
    "ydl-progress:",
    r#"{"status":"%(progress.status)s","#,
    r#""downloaded":%(progress.downloaded_bytes,0)d,"#,
    r#""total":%(progress.total_bytes,progress.total_bytes_estimate,0)d,"#,
    r#""speed":%(progress.speed,0)d,"#,
    r#""eta":%(progress.eta,0)d,"#,
    r#""title":%(info.title)j}"#
);

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

/// Just enumerate URLs (used by playlist/channel expansion). Returns one ID per line.
pub async fn flat_playlist_ids(ctx: &DownloadCtx<'_>, url: &str) -> Result<Vec<String>> {
    let out = Command::new(ctx.ytdlp_path)
        .arg("--flat-playlist")
        .arg("--print")
        .arg("%(url,webpage_url,id)s")
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
    Ok(stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
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

    args.extend(cfg.ytdlp.extra_args.iter().cloned());

    args.push(url.to_string());
    args
}

/// Download a single URL, driving `pb` with progress updates.
pub async fn download(ctx: &DownloadCtx<'_>, url: &str, pb: &ProgressBar) -> Result<()> {
    let args = build_args(ctx, url);
    if let Some(d) = ctx.cfg.defaults.output_dir.as_path().parent() {
        let _ = tokio::fs::create_dir_all(d).await;
    }
    let _ = tokio::fs::create_dir_all(&ctx.cfg.defaults.output_dir).await;

    pb.set_message("starting…");
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

    let pb_clone = pb.clone();
    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            handle_stdout_line(&line, &pb_clone);
        }
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

    let _ = stdout_task.await;
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
    pb.finish_with_message("done");
    Ok(())
}

fn handle_stdout_line(line: &str, pb: &ProgressBar) {
    if let Some(rest) = line.strip_prefix(PROGRESS_PREFIX) {
        match serde_json::from_str::<ProgressTick>(rest) {
            Ok(tick) => apply_tick(&tick, pb),
            Err(e) => tracing::trace!("progress parse error: {e} | line: {rest}"),
        }
        return;
    }
    if line.contains("[download]") && line.contains("has already been recorded") {
        pb.set_message("skipped (in archive)");
        pb.finish_with_message("skipped");
        return;
    }
    if line.contains("[Merger]") {
        pb.set_message("merging…");
    }
    tracing::debug!(target: "yt-dlp", "{}", line);
}

fn apply_tick(t: &ProgressTick, pb: &ProgressBar) {
    if !t.title.is_empty() {
        pb.set_message(t.title.clone());
    }
    match t.status.as_str() {
        "downloading" => {
            if t.total > 0 {
                pb.set_length(t.total);
            }
            pb.set_position(t.downloaded);
        }
        "finished" => {
            if t.total > 0 {
                pb.set_length(t.total);
                pb.set_position(t.total);
            }
        }
        "error" => {
            pb.set_message("error");
        }
        _ => {}
    }
    let _ = t.speed; // indicatif derives bytes_per_sec from positions
    let _ = t.eta;
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

// keep PathBuf in scope for the public surface; suppresses unused warnings on platforms
// that may not exercise every helper above.
#[allow(dead_code)]
fn _path_buf_keepalive(p: PathBuf) -> PathBuf {
    p
}

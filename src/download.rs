use crate::opts::DownloadOpts;
use crate::config::Config;
use crate::deps::{self, Tool};
use crate::error::{bail, Context, Result};
use crate::event::{DownloadEvent, EventSink};
use crate::summary::{Kind, Summary};
use crate::ytdlp::{self, DownloadCtx, DownloadOutcome};
use futures::stream::{FuturesUnordered, StreamExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Single,
    Playlist,
    Batch,
}

/// Classify a URL into the download mode it most likely needs.
///
/// Used by UIs that accept a single pasted URL and want to pick the right mode
/// automatically. Heuristic, not authoritative — yt-dlp has the final say.
pub fn classify_url(url: &str) -> Mode {
    let u = url.to_ascii_lowercase();
    // Channels and playlists both expand to a set of videos (Mode::Playlist).
    let is_collection = u.contains("/@")
        || u.contains("/channel/")
        || u.contains("/c/")
        || u.contains("/user/")
        || u.contains("list=")
        || u.contains("/playlist")
        || u.contains("youtube.com/playlist")
        || u.contains("music.youtube.com");
    if is_collection {
        Mode::Playlist
    } else {
        Mode::Single
    }
}

/// Run a download set, emitting progress through `sink`, and return the
/// aggregated [`Summary`]. This is the single orchestration entry point shared
/// by the CLI and the desktop app; it performs no rendering of its own.
///
/// When `cancel` is set and cancelled, no new workers are started and in-flight
/// tasks are aborted (yt-dlp children use `kill_on_drop`).
pub async fn run_with_sink<S: EventSink>(
    cfg: &Config,
    opts: &DownloadOpts,
    urls: Vec<String>,
    mode: Mode,
    sink: &S,
) -> Result<Summary> {
    run_with_sink_cancel(cfg, opts, urls, mode, sink, None).await
}

/// Same as [`run_with_sink`] with an optional cancellation token.
pub async fn run_with_sink_cancel<S: EventSink>(
    cfg: &Config,
    opts: &DownloadOpts,
    urls: Vec<String>,
    mode: Mode,
    sink: &S,
    cancel: Option<CancellationToken>,
) -> Result<Summary> {
    let start = Instant::now();

    if opts.dry_run {
        return run_dry(cfg, opts, urls, mode, sink).await;
    }

    let ytdlp_path = deps::ensure(cfg, Tool::YtDlp, opts.yes, opts.no_autoinstall)
        .await
        .context("ensure yt-dlp")?;

    let audio_only = cfg.defaults.audio_only || opts.audio_only;
    let ffmpeg_path = if audio_only {
        deps::ensure(cfg, Tool::Ffmpeg, opts.yes, opts.no_autoinstall)
            .await
            .ok()
    } else {
        Some(
            deps::ensure(cfg, Tool::Ffmpeg, opts.yes, opts.no_autoinstall)
                .await
                .map_err(|e| {
                    if cfg!(target_os = "macos") {
                        anyhow::anyhow!(
                            "{e:#}\n\nffmpeg is required for video downloads. On macOS install it with:\n  brew install ffmpeg"
                        )
                    } else {
                        e
                    }
                })
                .context("ensure ffmpeg")?,
        )
    };

    // For playlist/channel modes, expand to individual video URLs first so we can
    // drive the overall counter accurately and capture the playlist title.
    let mut playlist_title: Option<String> = None;
    let urls: Vec<String> = if matches!(mode, Mode::Playlist) && urls.len() == 1 {
        let ctx = DownloadCtx {
            cfg,
            ytdlp_path: &ytdlp_path,
            ffmpeg_path: ffmpeg_path.as_deref(),
        };
        let (title, ids) = ytdlp::expand_playlist(&ctx, &urls[0])
            .await
            .context("expand playlist/channel")?;
        if ids.is_empty() {
            bail!("playlist/channel expansion returned no videos");
        }
        playlist_title = title;
        ids
    } else {
        urls
    };

    if cancel.as_ref().is_some_and(|c| c.is_cancelled()) {
        bail!("download cancelled");
    }

    sink.emit(DownloadEvent::Expanded {
        total: urls.len(),
        playlist_title: playlist_title.clone(),
    });

    let jobs = cfg.parallel.jobs.max(1);
    let sem = Arc::new(Semaphore::new(jobs));
    let cfg_arc = Arc::new(cfg.clone());
    let ytdlp_path = Arc::new(ytdlp_path);
    let ffmpeg_path = ffmpeg_path.map(Arc::new);
    // Shared base args without the trailing URL — each worker appends its URL.
    let base_args = Arc::new(ytdlp::build_base_args(
        &DownloadCtx {
            cfg: &cfg_arc,
            ytdlp_path: ytdlp_path.as_path(),
            ffmpeg_path: ffmpeg_path.as_deref().map(|p: &PathBuf| p.as_path()),
        },
    ));

    let mut tasks = FuturesUnordered::new();
    let mut abort_handles = Vec::new();
    let mut cancelled_before_spawn = false;

    for (id, url) in urls.into_iter().enumerate() {
        if cancel.as_ref().is_some_and(|c| c.is_cancelled()) {
            cancelled_before_spawn = true;
            break;
        }

        let id = id as u64;
        let permit = if let Some(c) = &cancel {
            tokio::select! {
                biased;
                _ = c.cancelled() => {
                    cancelled_before_spawn = true;
                    break;
                }
                p = sem.clone().acquire_owned() => p?,
            }
        } else {
            sem.clone().acquire_owned().await?
        };

        let sink = sink.clone();
        let cfg_arc = cfg_arc.clone();
        let ytdlp_path = ytdlp_path.clone();
        let ffmpeg_path = ffmpeg_path.clone();
        let base_args = base_args.clone();
        let child_cancel = cancel.clone();

        let handle = tokio::spawn(async move {
            let _permit = permit;
            if child_cancel.as_ref().is_some_and(|c| c.is_cancelled()) {
                return (url, Err(anyhow::anyhow!("download cancelled")));
            }
            sink.emit(DownloadEvent::Started {
                id,
                url: url.clone(),
            });
            let ctx = DownloadCtx {
                cfg: &cfg_arc,
                ytdlp_path: ytdlp_path.as_path(),
                ffmpeg_path: ffmpeg_path.as_deref().map(|p: &PathBuf| p.as_path()),
            };
            let result = ytdlp::download_with_base_args(&ctx, &url, id, &sink, &base_args).await;
            match &result {
                Ok(o) => sink.emit(DownloadEvent::Completed {
                    id,
                    title: o.title.clone(),
                    path: o.path.as_ref().map(|p| p.display().to_string()),
                    bytes: o.bytes,
                    skipped: o.skipped,
                }),
                Err(e) => sink.emit(DownloadEvent::Failed {
                    id,
                    error: format!("{e}"),
                }),
            }
            (url, result)
        });
        abort_handles.push(handle.abort_handle());
        tasks.push(handle);
    }

    if cancelled_before_spawn {
        for a in &abort_handles {
            a.abort();
        }
        while tasks.next().await.is_some() {}
        bail!("download cancelled");
    }

    let mut downloaded_count = 0usize;
    let mut skipped_count = 0usize;
    let mut failed_count = 0usize;
    let mut total_bytes = 0u64;
    let mut single_outcome: Option<DownloadOutcome> = None;
    let mut single_url: Option<String> = None;
    let mut first_error: Option<String> = None;

    loop {
        let joined = if let Some(c) = &cancel {
            tokio::select! {
                biased;
                _ = c.cancelled() => {
                    for a in &abort_handles {
                        a.abort();
                    }
                    while tasks.next().await.is_some() {}
                    bail!("download cancelled");
                }
                j = tasks.next() => j,
            }
        } else {
            tasks.next().await
        };

        let Some(joined) = joined else { break };

        let (url, res) = match joined {
            Ok(pair) => pair,
            Err(e) if e.is_cancelled() => continue,
            Err(e) => return Err(e).context("worker task panicked"),
        };
        match res {
            Ok(o) => {
                if o.skipped {
                    skipped_count += 1;
                } else {
                    downloaded_count += 1;
                    total_bytes += o.bytes;
                }
                if matches!(mode, Mode::Single) {
                    single_url = Some(url.clone());
                    single_outcome = Some(o);
                }
            }
            Err(e) => {
                let msg = format!("{e:#}");
                if msg.contains("cancelled") {
                    continue;
                }
                failed_count += 1;
                if first_error.is_none() {
                    first_error = Some(msg);
                }
                if matches!(mode, Mode::Single) {
                    single_url = Some(url.clone());
                }
                tracing::error!("{url}: {e:#}");
            }
        }
    }

    let elapsed = start.elapsed();
    let summary = build_summary(
        cfg,
        mode,
        elapsed,
        false,
        playlist_title,
        downloaded_count,
        skipped_count,
        failed_count,
        total_bytes,
        single_outcome,
        single_url,
        first_error,
    );

    Ok(summary)
}

async fn run_dry<S: EventSink>(
    cfg: &Config,
    opts: &DownloadOpts,
    urls: Vec<String>,
    mode: Mode,
    _sink: &S,
) -> Result<Summary> {
    let mut playlist_title: Option<String> = None;
    let urls: Vec<String> = if matches!(mode, Mode::Playlist) && urls.len() == 1 {
        match deps::ensure(cfg, Tool::YtDlp, opts.yes, opts.no_autoinstall).await {
            Ok(ytdlp_path) => {
                let ctx = DownloadCtx {
                    cfg,
                    ytdlp_path: &ytdlp_path,
                    ffmpeg_path: None,
                };
                match ytdlp::expand_playlist(&ctx, &urls[0]).await {
                    Ok((title, ids)) if !ids.is_empty() => {
                        playlist_title = title;
                        ids
                    }
                    Ok(_) => bail!("playlist/channel expansion returned no videos"),
                    Err(e) => return Err(e).context("expand playlist/channel"),
                }
            }
            Err(_) => urls,
        }
    } else {
        urls
    };

    for u in &urls {
        println!("would download: {u}");
    }

    let summary = build_summary(
        cfg,
        mode,
        Duration::ZERO,
        true,
        playlist_title,
        urls.len(),
        0,
        0,
        0,
        None,
        urls.first().cloned(),
        None,
    );
    Ok(summary)
}

#[allow(clippy::too_many_arguments)]
fn build_summary(
    cfg: &Config,
    mode: Mode,
    elapsed: Duration,
    dry_run: bool,
    playlist_title: Option<String>,
    downloaded: usize,
    skipped: usize,
    failed: usize,
    total_bytes: u64,
    single_outcome: Option<DownloadOutcome>,
    single_url: Option<String>,
    first_error: Option<String>,
) -> Summary {
    let absolute_default = absolute_dir(&cfg.defaults.output_dir);

    match mode {
        Mode::Single => {
            let o = single_outcome.unwrap_or_default();
            let directory = o
                .path
                .as_ref()
                .and_then(|p| p.parent().map(absolute_dir))
                .unwrap_or_else(|| absolute_default.clone());
            Summary {
                directory,
                elapsed,
                dry_run,
                kind: Kind::Single {
                    title: o.title,
                    url: single_url,
                    bytes: o.bytes,
                    skipped: o.skipped,
                    failed: first_error,
                },
            }
        }
        Mode::Playlist | Mode::Batch => Summary {
            directory: absolute_default,
            elapsed,
            dry_run,
            kind: Kind::Multi {
                playlist_title,
                downloaded,
                skipped,
                failed,
                total_bytes,
            },
        },
    }
}

pub fn absolute_dir(p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    }
}

pub async fn read_batch_file(path: &Path) -> Result<Vec<String>> {
    let raw = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("read batch file {}", path.display()))?;
    Ok(raw
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_single_video() {
        assert!(matches!(
            classify_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            Mode::Single
        ));
    }

    #[test]
    fn classify_playlist_and_channel() {
        assert!(matches!(
            classify_url("https://www.youtube.com/playlist?list=PLxxx"),
            Mode::Playlist
        ));
        assert!(matches!(
            classify_url("https://www.youtube.com/@SomeChannel/videos"),
            Mode::Playlist
        ));
        assert!(matches!(
            classify_url("https://www.youtube.com/channel/UCxxx"),
            Mode::Playlist
        ));
    }
}

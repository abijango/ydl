use crate::cli::DownloadOpts;
use crate::config::Config;
use crate::deps::{self, Tool};
use crate::error::{Context, Result};
use crate::progress;
use crate::summary::{self, Kind, Summary};
use crate::ytdlp::{self, DownloadCtx, DownloadOutcome};
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::MultiProgress;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Single,
    Playlist,
    Batch,
}

pub async fn run(cfg: &Config, opts: &DownloadOpts, urls: Vec<String>, mode: Mode) -> Result<()> {
    let start = Instant::now();

    if opts.dry_run {
        return run_dry(cfg, opts, urls, mode).await;
    }

    let ytdlp_path = deps::ensure(cfg, Tool::YtDlp, opts.yes, opts.no_autoinstall)
        .await
        .context("ensure yt-dlp")?;
    let ffmpeg_path = deps::ensure(cfg, Tool::Ffmpeg, opts.yes, opts.no_autoinstall)
        .await
        .ok();

    // For playlist/channel modes, expand to individual video URLs first so we can
    // drive the overall counter accurately and capture the playlist title.
    let mut playlist_title: Option<String> = None;
    let urls: Vec<String> = if matches!(mode, Mode::Playlist) && urls.len() == 1 {
        let ctx = DownloadCtx {
            cfg,
            ytdlp_path: &ytdlp_path,
            ffmpeg_path: ffmpeg_path.as_deref(),
        };
        match ytdlp::expand_playlist(&ctx, &urls[0]).await {
            Ok((title, ids)) if !ids.is_empty() => {
                playlist_title = title;
                ids
            }
            _ => urls,
        }
    } else {
        urls
    };

    let total = urls.len() as u64;
    let mp = MultiProgress::new();
    let overall = if total > 1 {
        let pb = progress::make_overall_bar(&mp, total);
        pb.set_message("downloading…");
        Some(pb)
    } else {
        None
    };

    let jobs = cfg.parallel.jobs.max(1);
    let sem = Arc::new(Semaphore::new(jobs));
    let slot_pool: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new((0..jobs).collect()));
    let mp = Arc::new(mp);
    let cfg_arc = Arc::new(cfg.clone());
    let ytdlp_path = Arc::new(ytdlp_path);
    let ffmpeg_path = ffmpeg_path.map(Arc::new);

    let mut tasks = FuturesUnordered::new();
    for url in urls {
        let permit = sem.clone().acquire_owned().await?;
        let mp = mp.clone();
        let slot_pool = slot_pool.clone();
        let cfg_arc = cfg_arc.clone();
        let ytdlp_path = ytdlp_path.clone();
        let ffmpeg_path = ffmpeg_path.clone();

        tasks.push(tokio::spawn(async move {
            let _permit = permit;
            let slot = slot_pool.lock().await.pop().unwrap_or(0);
            let pb = progress::make_worker_bar(&mp, slot);
            let ctx = DownloadCtx {
                cfg: &cfg_arc,
                ytdlp_path: ytdlp_path.as_path(),
                ffmpeg_path: ffmpeg_path.as_deref().map(|p: &PathBuf| p.as_path()),
            };
            let result = ytdlp::download(&ctx, &url, &pb).await;
            match &result {
                Ok(o) if o.skipped => pb.finish_with_message(format!("⊘ {url} (skipped)")),
                Ok(_) => pb.finish_with_message(format!("✓ {url}")),
                Err(e) => pb.finish_with_message(format!("✗ {url} — {e}")),
            }
            slot_pool.lock().await.push(slot);
            (url, result)
        }));
    }

    let mut downloaded_count = 0usize;
    let mut skipped_count = 0usize;
    let mut failed_count = 0usize;
    let mut total_bytes = 0u64;
    let mut single_outcome: Option<DownloadOutcome> = None;
    let mut single_url: Option<String> = None;
    let mut first_error: Option<String> = None;

    while let Some(joined) = tasks.next().await {
        let (url, res) = joined.context("worker task panicked")?;
        if let Some(pb) = &overall {
            pb.inc(1);
        }
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
                failed_count += 1;
                if first_error.is_none() {
                    first_error = Some(format!("{e:#}"));
                }
                if matches!(mode, Mode::Single) {
                    single_url = Some(url.clone());
                }
                tracing::error!("{url}: {e:#}");
            }
        }
    }

    if let Some(pb) = overall {
        pb.finish_with_message(if failed_count == 0 {
            "all done".to_string()
        } else {
            format!("done with {failed_count} failure(s)")
        });
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
    summary::render(&summary);

    if failed_count > 0 {
        crate::error::bail!("{failed_count} download(s) failed");
    }
    Ok(())
}

async fn run_dry(cfg: &Config, opts: &DownloadOpts, urls: Vec<String>, mode: Mode) -> Result<()> {
    // Try to expand a playlist so the count and title are accurate; falls back
    // gracefully if yt-dlp isn't available yet.
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
                    _ => urls,
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
        urls.len(), // for dry_run, treat all as "would be downloaded"
        0,
        0,
        0,
        None,
        urls.first().cloned(),
        None,
    );
    summary::render(&summary);
    Ok(())
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

fn absolute_dir(p: &Path) -> PathBuf {
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

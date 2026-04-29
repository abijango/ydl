use crate::cli::DownloadOpts;
use crate::config::Config;
use crate::deps::{self, Tool};
use crate::error::{Context, Result};
use crate::progress;
use crate::ytdlp::{self, DownloadCtx};
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::MultiProgress;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Single,
    Playlist,
    Batch,
}

pub async fn run(cfg: &Config, opts: &DownloadOpts, urls: Vec<String>, mode: Mode) -> Result<()> {
    if opts.dry_run {
        for u in &urls {
            println!("would download: {u}");
        }
        return Ok(());
    }

    let ytdlp_path = deps::ensure(cfg, Tool::YtDlp, opts.yes, opts.no_autoinstall)
        .await
        .context("ensure yt-dlp")?;
    let ffmpeg_path = deps::ensure(cfg, Tool::Ffmpeg, opts.yes, opts.no_autoinstall)
        .await
        .ok();

    // For playlist/channel modes, expand to individual video URLs first so we can
    // drive the overall counter accurately.
    let urls: Vec<String> = if matches!(mode, Mode::Playlist) && urls.len() == 1 {
        let ctx = DownloadCtx {
            cfg,
            ytdlp_path: &ytdlp_path,
            ffmpeg_path: ffmpeg_path.as_deref(),
        };
        match ytdlp::flat_playlist_ids(&ctx, &urls[0]).await {
            Ok(ids) if !ids.is_empty() => ids,
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
    let cfg = Arc::new(cfg.clone());
    let ytdlp_path = Arc::new(ytdlp_path);
    let ffmpeg_path = ffmpeg_path.map(Arc::new);

    let mut tasks = FuturesUnordered::new();
    for url in urls {
        let permit = sem.clone().acquire_owned().await?;
        let mp = mp.clone();
        let slot_pool = slot_pool.clone();
        let cfg = cfg.clone();
        let ytdlp_path = ytdlp_path.clone();
        let ffmpeg_path = ffmpeg_path.clone();

        tasks.push(tokio::spawn(async move {
            let _permit = permit;
            let slot = slot_pool.lock().await.pop().unwrap_or(0);
            let pb = progress::make_worker_bar(&mp, slot);
            let ctx = DownloadCtx {
                cfg: &cfg,
                ytdlp_path: ytdlp_path.as_path(),
                ffmpeg_path: ffmpeg_path.as_deref().map(|p: &PathBuf| p.as_path()),
            };
            let result = ytdlp::download(&ctx, &url, &pb).await;
            match &result {
                Ok(_) => pb.finish_with_message(format!("✓ {url}")),
                Err(e) => pb.finish_with_message(format!("✗ {url} — {e}")),
            }
            slot_pool.lock().await.push(slot);
            (url, result)
        }));
    }

    let mut failures = 0usize;
    while let Some(joined) = tasks.next().await {
        let (url, res) = joined.context("worker task panicked")?;
        if let Some(pb) = &overall {
            pb.inc(1);
        }
        if let Err(e) = res {
            failures += 1;
            tracing::error!("{url}: {e:#}");
        }
    }

    if let Some(pb) = overall {
        pb.finish_with_message(if failures == 0 {
            "all done".to_string()
        } else {
            format!("done with {failures} failure(s)")
        });
    }

    if failures > 0 {
        crate::error::bail!("{failures} download(s) failed");
    }
    Ok(())
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

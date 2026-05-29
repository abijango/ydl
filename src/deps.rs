use crate::config::{self, Config};
use crate::error::{anyhow, bail, Context, Result};
use crate::progress;
use futures::StreamExt;
use indicatif::MultiProgress;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const USER_AGENT: &str = concat!("ydl/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    YtDlp,
    Ffmpeg,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Tool::YtDlp => "yt-dlp",
            Tool::Ffmpeg => "ffmpeg",
        }
    }

    pub fn binary_name(self) -> &'static str {
        match (self, cfg!(windows)) {
            (Tool::YtDlp, true) => "yt-dlp.exe",
            (Tool::YtDlp, false) => "yt-dlp",
            (Tool::Ffmpeg, true) => "ffmpeg.exe",
            (Tool::Ffmpeg, false) => "ffmpeg",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VersionsManifest {
    #[serde(default)]
    pub yt_dlp: Option<InstalledVersion>,
    #[serde(default)]
    pub ffmpeg: Option<InstalledVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledVersion {
    pub version: String,
    pub installed_at: String,
    pub source: String,
}

pub fn manifest_path() -> Result<PathBuf> {
    Ok(config::bin_dir()?.join("versions.json"))
}

pub async fn load_manifest() -> Result<VersionsManifest> {
    let p = manifest_path()?;
    if !p.exists() {
        return Ok(VersionsManifest::default());
    }
    let raw = fs::read_to_string(&p).await.with_context(|| format!("read {}", p.display()))?;
    let m: VersionsManifest =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", p.display()))?;
    Ok(m)
}

pub async fn save_manifest(m: &VersionsManifest) -> Result<()> {
    let p = manifest_path()?;
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).await?;
    }
    let raw = serde_json::to_string_pretty(m)?;
    fs::write(&p, raw).await?;
    Ok(())
}

/// Resolve the path that should be used to invoke `tool`, applying the precedence:
/// 1. explicit absolute path in config
/// 2. managed binary under data_dir/bin
/// 3. binary on PATH
/// Returns None if none of the above resolve.
pub fn resolve(cfg: &Config, tool: Tool) -> Result<Option<PathBuf>> {
    let explicit = match tool {
        Tool::YtDlp => cfg.ytdlp.binary.trim(),
        Tool::Ffmpeg => cfg.ffmpeg.binary.trim(),
    };
    if !explicit.is_empty() {
        let p = PathBuf::from(explicit);
        if p.is_absolute() && p.exists() {
            return Ok(Some(p));
        }
    }
    let managed = config::bin_dir()?.join(tool.binary_name());
    if managed.exists() {
        return Ok(Some(managed));
    }
    if let Some(p) = which_on_path(tool.binary_name()) {
        return Ok(Some(p));
    }
    Ok(None)
}

fn which_on_path(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    // macOS GUI apps launched from Finder/Dock inherit a minimal PATH that omits
    // Homebrew, so also probe the common install locations. Harmless elsewhere
    // (these dirs simply won't exist on Windows).
    for extra in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
        let p = PathBuf::from(extra);
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    }
    for dir in dirs {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Make sure `tool` resolves to a usable binary, installing it if necessary
/// and permitted by config + the `assume_yes` / `no_autoinstall` flags.
pub async fn ensure(
    cfg: &Config,
    tool: Tool,
    assume_yes: bool,
    no_autoinstall: bool,
) -> Result<PathBuf> {
    if let Some(p) = resolve(cfg, tool)? {
        return Ok(p);
    }
    if no_autoinstall || !cfg.ytdlp.auto_install {
        bail!(
            "{} not found on PATH or under managed bin dir, and auto-install is disabled",
            tool.label()
        );
    }
    let target = config::bin_dir()?.join(tool.binary_name());
    if !assume_yes && atty_stdin() {
        eprintln!(
            "{} not found. Install to {}? [Y/n] ",
            tool.label(),
            target.display()
        );
        if !read_yes_no_default_yes() {
            bail!("install of {} declined by user", tool.label());
        }
    } else {
        eprintln!("{} not found — installing to {}", tool.label(), target.display());
    }
    install(tool).await?;
    resolve(cfg, tool)?
        .ok_or_else(|| anyhow!("{} install reported success but binary missing", tool.label()))
}

fn atty_stdin() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

fn read_yes_no_default_yes() -> bool {
    let mut buf = String::new();
    if std::io::stdin().read_line(&mut buf).is_err() {
        return true;
    }
    let s = buf.trim().to_ascii_lowercase();
    matches!(s.as_str(), "" | "y" | "yes")
}

/// Install (or reinstall) a tool. Always overwrites the managed copy.
pub async fn install(tool: Tool) -> Result<()> {
    let bin_dir = config::bin_dir()?;
    fs::create_dir_all(&bin_dir).await?;

    let target = bin_dir.join(tool.binary_name());
    let mp = MultiProgress::new();

    match tool {
        Tool::YtDlp => install_ytdlp(&mp, &target).await?,
        Tool::Ffmpeg => install_ffmpeg(&mp, &bin_dir).await?,
    }

    let version = read_version(&target).await.unwrap_or_else(|_| "unknown".to_string());
    let mut m = load_manifest().await.unwrap_or_default();
    let entry = InstalledVersion {
        version,
        installed_at: now_rfc3339(),
        source: source_url_for(tool)?,
    };
    match tool {
        Tool::YtDlp => m.yt_dlp = Some(entry),
        Tool::Ffmpeg => m.ffmpeg = Some(entry),
    }
    save_manifest(&m).await?;
    eprintln!("✓ installed {} at {}", tool.label(), target.display());
    Ok(())
}

pub async fn update_all() -> Result<()> {
    install(Tool::YtDlp).await?;
    install(Tool::Ffmpeg).await?;
    Ok(())
}

pub async fn status(cfg: &Config) -> Result<()> {
    let manifest = load_manifest().await.unwrap_or_default();
    for tool in [Tool::YtDlp, Tool::Ffmpeg] {
        let resolved = resolve(cfg, tool)?;
        let manifest_entry = match tool {
            Tool::YtDlp => manifest.yt_dlp.as_ref(),
            Tool::Ffmpeg => manifest.ffmpeg.as_ref(),
        };
        match resolved {
            Some(p) => {
                let v = read_version(&p).await.unwrap_or_else(|_| "(unknown)".into());
                println!("{:<8} {}", tool.label(), p.display());
                println!("         version: {v}");
                if let Some(m) = manifest_entry {
                    println!("         managed: {} (installed {})", m.version, m.installed_at);
                }
            }
            None => {
                println!("{:<8} (not installed)", tool.label());
            }
        }
    }
    Ok(())
}

async fn read_version(path: &Path) -> Result<String> {
    let arg = if path.file_stem().and_then(|s| s.to_str()) == Some("ffmpeg") {
        "-version"
    } else {
        "--version"
    };
    let out = Command::new(path)
        .arg(arg)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("invoke {} {arg}", path.display()))?;
    let s = String::from_utf8_lossy(&out.stdout);
    let first = s.lines().next().unwrap_or("").trim().to_string();
    Ok(first)
}

fn now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // simple "epoch=N" stamp; avoids pulling in chrono just for this
    format!("epoch={secs}")
}

fn source_url_for(tool: Tool) -> Result<String> {
    Ok(match tool {
        Tool::YtDlp => ytdlp_asset_url()?,
        Tool::Ffmpeg => ffmpeg_asset_url()?,
    })
}

// ---------- yt-dlp ----------

fn ytdlp_asset_url() -> Result<String> {
    let asset = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", _) => "yt-dlp.exe",
        ("macos", _) => "yt-dlp_macos",
        ("linux", "aarch64") => "yt-dlp_linux_aarch64",
        ("linux", _) => "yt-dlp_linux",
        (os, arch) => bail!("no yt-dlp release asset for {os}/{arch}"),
    };
    Ok(format!(
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/{asset}"
    ))
}

async fn install_ytdlp(mp: &MultiProgress, target: &Path) -> Result<()> {
    let url = ytdlp_asset_url()?;
    let tmp = target.with_extension("tmp");
    download_with_progress(&url, &tmp, mp, "yt-dlp").await?;
    if target.exists() {
        let _ = fs::remove_file(target).await;
    }
    fs::rename(&tmp, target).await.with_context(|| format!("rename to {}", target.display()))?;
    chmod_exec(target).await?;
    Ok(())
}

// ---------- ffmpeg ----------

enum FfmpegSource {
    BtbnZip(String),   // windows
    BtbnTarXz(String), // linux
    RawBinary(String), // macOS — a direct ffmpeg binary, no archive to extract
}

fn ffmpeg_asset_url() -> Result<String> {
    Ok(match ffmpeg_source()? {
        FfmpegSource::BtbnZip(u) | FfmpegSource::BtbnTarXz(u) | FfmpegSource::RawBinary(u) => u,
    })
}

fn ffmpeg_source() -> Result<FfmpegSource> {
    let base = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest";
    // macOS: BtbN ships no Mac build, so pull an arch-native static binary from
    // eugeneware/ffmpeg-static (direct executables, no archive).
    let mac_base = "https://github.com/eugeneware/ffmpeg-static/releases/latest/download";
    Ok(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", _) => FfmpegSource::BtbnZip(format!(
            "{base}/ffmpeg-master-latest-win64-gpl.zip"
        )),
        ("linux", "aarch64") => FfmpegSource::BtbnTarXz(format!(
            "{base}/ffmpeg-master-latest-linuxarm64-gpl.tar.xz"
        )),
        ("linux", _) => FfmpegSource::BtbnTarXz(format!(
            "{base}/ffmpeg-master-latest-linux64-gpl.tar.xz"
        )),
        ("macos", "aarch64") => FfmpegSource::RawBinary(format!("{mac_base}/ffmpeg-darwin-arm64")),
        ("macos", _) => FfmpegSource::RawBinary(format!("{mac_base}/ffmpeg-darwin-x64")),
        (os, arch) => return Err(anyhow!("ffmpeg auto-install: unsupported platform {os}/{arch}")),
    })
}

async fn install_ffmpeg(mp: &MultiProgress, bin_dir: &Path) -> Result<()> {
    let target = bin_dir.join(Tool::Ffmpeg.binary_name());
    match ffmpeg_source()? {
        FfmpegSource::BtbnZip(url) => {
            let tmp = bin_dir.join("ffmpeg-archive.zip");
            download_with_progress(&url, &tmp, mp, "ffmpeg").await?;
            extract_ffmpeg_zip(&tmp, &target).await?;
            let _ = fs::remove_file(&tmp).await;
        }
        FfmpegSource::BtbnTarXz(url) => {
            let tmp = bin_dir.join("ffmpeg-archive.tar.xz");
            download_with_progress(&url, &tmp, mp, "ffmpeg").await?;
            extract_ffmpeg_tar_xz(&tmp, &target).await?;
            let _ = fs::remove_file(&tmp).await;
        }
        FfmpegSource::RawBinary(url) => {
            // The asset is the ffmpeg executable itself — download and move into place.
            let tmp = target.with_extension("tmp");
            download_with_progress(&url, &tmp, mp, "ffmpeg").await?;
            if target.exists() {
                let _ = fs::remove_file(&target).await;
            }
            fs::rename(&tmp, &target)
                .await
                .with_context(|| format!("rename to {}", target.display()))?;
        }
    }
    chmod_exec(&target).await?;
    Ok(())
}

async fn extract_ffmpeg_zip(archive: &Path, target: &Path) -> Result<()> {
    let archive = archive.to_path_buf();
    let target = target.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let f = std::fs::File::open(&archive)
            .with_context(|| format!("open {}", archive.display()))?;
        let mut zip = zip::ZipArchive::new(f).context("open ffmpeg zip")?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let name = entry.name().to_string();
            let lower = name.to_ascii_lowercase();
            if lower.ends_with("/bin/ffmpeg.exe") || lower.ends_with("\\bin\\ffmpeg.exe") {
                let mut out = std::fs::File::create(&target)
                    .with_context(|| format!("create {}", target.display()))?;
                std::io::copy(&mut entry, &mut out)?;
                return Ok(());
            }
        }
        Err(anyhow!("ffmpeg.exe not found inside zip"))
    })
    .await
    .context("extract ffmpeg zip task")?
}

async fn extract_ffmpeg_tar_xz(archive: &Path, target: &Path) -> Result<()> {
    let archive = archive.to_path_buf();
    let target = target.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let f = std::fs::File::open(&archive)?;
        let xz = xz2::read::XzDecoder::new(f);
        let mut tar = tar::Archive::new(xz);
        for entry in tar.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            let name = path.to_string_lossy().to_ascii_lowercase();
            if name.ends_with("/bin/ffmpeg") {
                let mut out = std::fs::File::create(&target)?;
                std::io::copy(&mut entry, &mut out)?;
                return Ok(());
            }
        }
        Err(anyhow!("ffmpeg not found inside tar.xz"))
    })
    .await
    .context("extract ffmpeg tar.xz task")?
}

// ---------- shared download ----------

async fn download_with_progress(
    url: &str,
    target: &Path,
    mp: &MultiProgress,
    label: &str,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?;
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("GET {url} returned error status"))?;

    let total = resp.content_length().unwrap_or(0);
    let pb = progress::make_install_bar(mp, label);
    pb.set_length(total);

    if let Some(d) = target.parent() {
        fs::create_dir_all(d).await?;
    }
    let mut file = fs::File::create(target).await?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("download chunk")?;
        file.write_all(&chunk).await?;
        pb.inc(chunk.len() as u64);
    }
    file.flush().await?;
    pb.finish_and_clear();
    Ok(())
}

#[cfg(unix)]
async fn chmod_exec(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path).await?.permissions();
    perms.set_mode(perms.mode() | 0o111);
    fs::set_permissions(path, perms).await?;
    Ok(())
}

#[cfg(not(unix))]
async fn chmod_exec(_path: &Path) -> Result<()> {
    Ok(())
}

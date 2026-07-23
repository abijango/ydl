use crate::config::{self, Config};
use crate::error::{anyhow, bail, Context, Result};
use crate::progress;
use futures::StreamExt;
use indicatif::MultiProgress;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::LazyLock;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::Mutex;

const USER_AGENT: &str = concat!("ydl/", env!("CARGO_PKG_VERSION"));

static INSTALL_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

struct InstallResult {
    source: String,
    sha256: String,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
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
    let tmp = p.with_file_name("versions.json.tmp");
    fs::write(&tmp, raw).await?;
    fs::rename(&tmp, &p).await?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

async fn hash_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).await.with_context(|| format!("read {}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

fn manifest_entry(manifest: &VersionsManifest, tool: Tool) -> Option<&InstalledVersion> {
    match tool {
        Tool::YtDlp => manifest.yt_dlp.as_ref(),
        Tool::Ffmpeg => manifest.ffmpeg.as_ref(),
    }
}

async fn verify_managed_hash(tool: Tool) -> Result<()> {
    let managed = config::bin_dir()?.join(tool.binary_name());
    if !managed.exists() {
        return Ok(());
    }
    let manifest = load_manifest().await?;
    let Some(entry) = manifest_entry(&manifest, tool) else {
        return Ok(());
    };
    let Some(expected) = &entry.sha256 else {
        return Ok(());
    };
    let actual = hash_file(&managed).await?;
    if actual != *expected {
        bail!(
            "{} checksum mismatch (expected {expected}, got {actual})",
            tool.label()
        );
    }
    Ok(())
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("build HTTP client")
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("GET {url} returned error status"))?;
    resp.text()
        .await
        .with_context(|| format!("read body from {url}"))
}

fn parse_sha256sums(content: &str, filename: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');
        if name == filename {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

/// Resolve the path that should be used to invoke `tool`, applying the precedence:
/// 1. explicit absolute path in config
/// 2. managed binary under data_dir/bin
/// 3. binary on PATH
///
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
        let managed = config::bin_dir()?.join(tool.binary_name());
        if p == managed {
            if let Err(e) = verify_managed_hash(tool).await {
                if no_autoinstall || !cfg.ytdlp.auto_install {
                    bail!("{e}");
                }
                eprintln!("{e} — reinstalling {}", tool.label());
                install(tool).await?;
                return resolve(cfg, tool)?
                    .ok_or_else(|| anyhow!("{} reinstall reported success but binary missing", tool.label()));
            }
        }
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
    let _guard = INSTALL_LOCK.lock().await;

    let bin_dir = config::bin_dir()?;
    fs::create_dir_all(&bin_dir).await?;

    let target = bin_dir.join(tool.binary_name());
    let mp = MultiProgress::new();

    let install_result = match tool {
        Tool::YtDlp => install_ytdlp(&mp, &target).await?,
        Tool::Ffmpeg => install_ffmpeg(&mp, &bin_dir).await?,
    };

    let sha256 = hash_file(&target).await?;
    if sha256 != install_result.sha256 {
        bail!(
            "{} post-install checksum mismatch (expected {}, got {})",
            tool.label(),
            install_result.sha256,
            sha256
        );
    }

    let version = read_version(&target).await.unwrap_or_else(|_| "unknown".to_string());
    let mut m = load_manifest().await.unwrap_or_default();
    let entry = InstalledVersion {
        version,
        installed_at: now_rfc3339(),
        source: install_result.source,
        sha256: Some(sha256),
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
        let manifest_entry = manifest_entry(&manifest, tool);
        match resolved {
            Some(p) => {
                let v = read_version(&p).await.unwrap_or_else(|_| "(unknown)".into());
                println!("{:<8} {}", tool.label(), p.display());
                println!("         version: {v}");
                if let Some(m) = manifest_entry {
                    println!("         managed: {} (installed {})", m.version, m.installed_at);
                    if let Some(hash) = &m.sha256 {
                        println!("         sha256: {hash}");
                    }
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
    let is_ffmpeg = path.file_stem().and_then(|s| s.to_str()) == Some("ffmpeg");
    let arg = if is_ffmpeg { "-version" } else { "--version" };
    let out = Command::new(path)
        .arg(arg)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("invoke {} {arg}", path.display()))?;
    let s = String::from_utf8_lossy(&out.stdout);
    let first = s.lines().next().unwrap_or("").trim();
    let tool = if is_ffmpeg { Tool::Ffmpeg } else { Tool::YtDlp };
    Ok(short_version(tool, first))
}

/// Condense a tool's raw `--version` / `-version` first line into a concise,
/// human-readable string fit for a status pill. ffmpeg prints
/// `ffmpeg version <TOKEN> Copyright (c) ... the FFmpeg developers`; we keep
/// only `<TOKEN>`. yt-dlp already prints a bare version, so it's passed through.
pub fn short_version(tool: Tool, raw: &str) -> String {
    let raw = raw.trim();
    match tool {
        Tool::Ffmpeg => raw
            .strip_prefix("ffmpeg version ")
            .and_then(|rest| rest.split_whitespace().next())
            .unwrap_or(raw)
            .to_string(),
        Tool::YtDlp => raw.to_string(),
    }
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

// ---------- yt-dlp ----------

fn ytdlp_asset_name() -> Result<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", _) => Ok("yt-dlp.exe"),
        ("macos", _) => Ok("yt-dlp_macos"),
        ("linux", "aarch64") => Ok("yt-dlp_linux_aarch64"),
        ("linux", _) => Ok("yt-dlp_linux"),
        (os, arch) => bail!("no yt-dlp release asset for {os}/{arch}"),
    }
}

async fn fetch_ytdlp_release(client: &reqwest::Client) -> Result<(String, String)> {
    let release: GhRelease = client
        .get("https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest")
        .send()
        .await
        .context("GET yt-dlp latest release")?
        .error_for_status()
        .context("yt-dlp latest release returned error status")?
        .json()
        .await
        .context("parse yt-dlp release JSON")?;

    let asset_name = ytdlp_asset_name()?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| anyhow!("yt-dlp release {} has no asset {asset_name}", release.tag_name))?;

    Ok((asset.browser_download_url.clone(), release.tag_name))
}

async fn verify_ytdlp_checksum(
    client: &reqwest::Client,
    tag: &str,
    asset_name: &str,
    local_hash: &str,
) -> Result<()> {
    let sums_url = format!(
        "https://github.com/yt-dlp/yt-dlp/releases/download/{tag}/SHA2-256SUMS"
    );
    match fetch_text(client, &sums_url).await {
        Ok(content) => {
            let expected = parse_sha256sums(&content, asset_name).ok_or_else(|| {
                anyhow!("SHA2-256SUMS for {tag} does not list {asset_name}")
            })?;
            if expected != local_hash {
                bail!(
                    "yt-dlp checksum mismatch for {asset_name} (expected {expected}, got {local_hash})"
                );
            }
        }
        Err(e) => {
            eprintln!(
                "warning: could not fetch yt-dlp SHA2-256SUMS for {tag}: {e}; stored local hash only"
            );
        }
    }
    Ok(())
}

async fn install_ytdlp(mp: &MultiProgress, target: &Path) -> Result<InstallResult> {
    let client = http_client()?;
    let asset_name = ytdlp_asset_name()?;
    let (url, tag) = fetch_ytdlp_release(&client).await?;

    let tmp = target.with_extension("tmp");
    download_with_progress(&url, &tmp, mp, "yt-dlp").await?;

    let sha256 = hash_file(&tmp).await?;
    verify_ytdlp_checksum(&client, &tag, asset_name, &sha256).await?;

    if target.exists() {
        let _ = fs::remove_file(target).await;
    }
    fs::rename(&tmp, target)
        .await
        .with_context(|| format!("rename to {}", target.display()))?;
    chmod_exec(target).await?;

    Ok(InstallResult { source: url, sha256 })
}

// ---------- ffmpeg ----------

enum FfmpegSource {
    BtbnZip(String),   // windows
    BtbnTarXz(String), // linux
    RawBinary(String), // macOS — a direct ffmpeg binary, no archive to extract
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

async fn install_ffmpeg(mp: &MultiProgress, bin_dir: &Path) -> Result<InstallResult> {
    let target = bin_dir.join(Tool::Ffmpeg.binary_name());
    let url = match ffmpeg_source()? {
        FfmpegSource::BtbnZip(url) => {
            let tmp = bin_dir.join("ffmpeg-archive.zip");
            download_with_progress(&url, &tmp, mp, "ffmpeg").await?;
            let meta = fs::metadata(&tmp).await?;
            if meta.len() == 0 {
                bail!("ffmpeg archive download is empty");
            }
            extract_ffmpeg_zip(&tmp, &target).await?;
            let _ = fs::remove_file(&tmp).await;
            url
        }
        FfmpegSource::BtbnTarXz(url) => {
            let tmp = bin_dir.join("ffmpeg-archive.tar.xz");
            download_with_progress(&url, &tmp, mp, "ffmpeg").await?;
            let meta = fs::metadata(&tmp).await?;
            if meta.len() == 0 {
                bail!("ffmpeg archive download is empty");
            }
            extract_ffmpeg_tar_xz(&tmp, &target).await?;
            let _ = fs::remove_file(&tmp).await;
            url
        }
        FfmpegSource::RawBinary(url) => {
            let tmp = target.with_extension("tmp");
            download_with_progress(&url, &tmp, mp, "ffmpeg").await?;
            if target.exists() {
                let _ = fs::remove_file(&target).await;
            }
            fs::rename(&tmp, &target)
                .await
                .with_context(|| format!("rename to {}", target.display()))?;
            url
        }
    };
    chmod_exec(&target).await?;
    let sha256 = hash_file(&target).await?;
    Ok(InstallResult { source: url, sha256 })
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
    let client = http_client()?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

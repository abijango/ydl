use crate::cli::DownloadOpts;
use crate::error::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default)]
    pub parallel: Parallel,
    #[serde(default)]
    pub archive: Archive,
    #[serde(default)]
    pub ytdlp: YtDlpCfg,
    #[serde(default)]
    pub ffmpeg: FfmpegCfg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    pub output_dir: PathBuf,
    pub filename_template: String,
    pub quality: String,
    pub merge_format: String,
    pub audio_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parallel {
    pub jobs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Archive {
    pub enabled: bool,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtDlpCfg {
    pub binary: String,
    pub auto_install: bool,
    pub auto_update: bool,
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FfmpegCfg {
    pub binary: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            defaults: Defaults::default(),
            parallel: Parallel::default(),
            archive: Archive::default(),
            ytdlp: YtDlpCfg::default(),
            ffmpeg: FfmpegCfg::default(),
        }
    }
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("."),
            filename_template: "{upload_date}-{title}.{ext}".to_string(),
            quality: "bv*+ba/b".to_string(),
            merge_format: "mp4".to_string(),
            audio_only: false,
        }
    }
}

impl Default for Parallel {
    fn default() -> Self {
        Self { jobs: 3 }
    }
}

impl Default for Archive {
    fn default() -> Self {
        Self {
            enabled: true,
            path: PathBuf::from(".ydl-archive"),
        }
    }
}

impl Default for YtDlpCfg {
    fn default() -> Self {
        Self {
            binary: String::new(),
            auto_install: true,
            auto_update: false,
            extra_args: Vec::new(),
        }
    }
}

impl Default for FfmpegCfg {
    fn default() -> Self {
        Self {
            binary: String::new(),
        }
    }
}

pub fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "", "ydl")
        .context("could not resolve OS-standard directories for ydl")
}

pub fn config_path() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().join("config.toml"))
}

pub fn data_dir() -> Result<PathBuf> {
    Ok(project_dirs()?.data_dir().to_path_buf())
}

pub fn bin_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("bin"))
}

pub fn load_or_init() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        write_default(&path)?;
    }
    load(&path)
}

pub fn load(path: &Path) -> Result<Config> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read config at {}", path.display()))?;
    let cfg: Config = toml::from_str(&raw)
        .with_context(|| format!("parse config at {}", path.display()))?;
    Ok(cfg)
}

pub fn write_default(path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("create config dir {}", dir.display()))?;
    }
    let cfg = Config::default();
    let toml = toml::to_string_pretty(&cfg).context("serialize default config")?;
    std::fs::write(path, toml).with_context(|| format!("write config to {}", path.display()))?;
    Ok(())
}

/// Apply CLI overrides on top of the loaded config.
pub fn merge_opts(cfg: &mut Config, opts: &DownloadOpts) {
    if let Some(d) = &opts.output_dir {
        cfg.defaults.output_dir = d.clone();
    }
    if let Some(j) = opts.jobs {
        cfg.parallel.jobs = j.max(1);
    }
    if let Some(q) = &opts.quality {
        cfg.defaults.quality = q.clone();
    }
    if let Some(m) = &opts.merge_format {
        cfg.defaults.merge_format = m.clone();
    }
    if opts.audio_only {
        cfg.defaults.audio_only = true;
    }
    if let Some(t) = &opts.filename_template {
        cfg.defaults.filename_template = t.clone();
    }
    if opts.no_archive {
        cfg.archive.enabled = false;
    }
    if let Some(a) = &opts.archive {
        cfg.archive.path = a.clone();
    }
}

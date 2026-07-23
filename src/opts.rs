use std::path::PathBuf;

/// Download options shared by the CLI, config merge, and GUI.
#[derive(Debug, Clone, Default)]
pub struct DownloadOpts {
    pub output_dir: Option<PathBuf>,
    pub jobs: Option<usize>,
    pub quality: Option<String>,
    pub merge_format: Option<String>,
    pub audio_only: bool,
    pub filename_template: Option<String>,
    pub no_archive: bool,
    pub archive: Option<PathBuf>,
    pub dry_run: bool,
    pub update: bool,
    pub yes: bool,
    pub no_autoinstall: bool,
}

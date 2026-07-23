#[cfg(feature = "cli")]
use clap::{Args, Parser, Subcommand};
use crate::opts::DownloadOpts as CoreDownloadOpts;
use std::path::PathBuf;

#[cfg(feature = "cli")]
#[derive(Parser, Debug)]
#[command(
    name = "ydl",
    version,
    about = "YouTube downloader built on yt-dlp + ffmpeg",
    long_about = "Pass a URL directly (e.g. `ydl <URL>`) to download a single video. \
                  Use the explicit subcommands for playlists, channels, batch files, \
                  config, or dependency management.",
    arg_required_else_help = true,
)]
pub struct Cli {
    /// Video URL to download. Ignored when a subcommand is given.
    pub url: Option<String>,

    /// Options applied when a bare URL is given. Subcommands have their own copies.
    #[command(flatten)]
    pub opts: CliDownloadOpts,

    #[command(subcommand)]
    pub command: Option<Command>,

    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
}

#[cfg(feature = "cli")]
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Download every video in a playlist
    Playlist {
        url: String,
        #[command(flatten)]
        opts: CliDownloadOpts,
    },
    /// Download every video on a channel (URL or @handle)
    Channel {
        url: String,
        #[command(flatten)]
        opts: CliDownloadOpts,
    },
    /// Download URLs listed (one per line) in FILE
    Batch {
        file: PathBuf,
        #[command(flatten)]
        opts: CliDownloadOpts,
    },
    /// Manage the TOML config
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage the bundled yt-dlp + ffmpeg binaries
    Deps {
        #[command(subcommand)]
        action: DepsAction,
    },
}

#[cfg(feature = "cli")]
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Print the resolved config
    Show,
    /// Print the path to the config file
    Path,
    /// Open the config file in $EDITOR
    Edit,
    /// (Re)generate the config file with defaults
    Init,
}

#[cfg(feature = "cli")]
#[derive(Subcommand, Debug)]
pub enum DepsAction {
    /// Show installed versions and resolved binary paths
    Status,
    /// Install any missing binaries
    Install,
    /// Re-check latest releases and update if newer
    Update,
}

#[cfg(feature = "cli")]
#[derive(Args, Debug, Clone, Default)]
pub struct CliDownloadOpts {
    /// Output directory (overrides config)
    #[arg(short = 'o', long)]
    pub output_dir: Option<PathBuf>,

    /// Number of parallel download workers
    #[arg(short = 'j', long)]
    pub jobs: Option<usize>,

    /// yt-dlp format selector, e.g. "bv*+ba/b"
    #[arg(short = 'q', long)]
    pub quality: Option<String>,

    /// Container for merged output
    #[arg(long)]
    pub merge_format: Option<String>,

    /// Download audio only (m4a)
    #[arg(long)]
    pub audio_only: bool,

    /// Filename template (e.g. "{upload_date}-{title}.{ext}")
    #[arg(long)]
    pub filename_template: Option<String>,

    /// Disable the incremental download archive
    #[arg(long)]
    pub no_archive: bool,

    /// Custom archive file path
    #[arg(long)]
    pub archive: Option<PathBuf>,

    /// List what would be downloaded without fetching
    #[arg(long)]
    pub dry_run: bool,

    /// Update yt-dlp + ffmpeg before running this command
    #[arg(long)]
    pub update: bool,

    /// Assume "yes" to interactive prompts (e.g. first-run install)
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Skip auto-installing missing dependencies
    #[arg(long)]
    pub no_autoinstall: bool,
}

#[cfg(feature = "cli")]
impl From<CliDownloadOpts> for CoreDownloadOpts {
    fn from(o: CliDownloadOpts) -> Self {
        Self {
            output_dir: o.output_dir,
            jobs: o.jobs,
            quality: o.quality,
            merge_format: o.merge_format,
            audio_only: o.audio_only,
            filename_template: o.filename_template,
            no_archive: o.no_archive,
            archive: o.archive,
            dry_run: o.dry_run,
            update: o.update,
            yes: o.yes,
            no_autoinstall: o.no_autoinstall,
        }
    }
}

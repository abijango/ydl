use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "ydl",
    version,
    about = "YouTube downloader built on yt-dlp + ffmpeg",
    long_about = "If invoked with just a URL (e.g. `ydl <URL>`), behaves like \
                  `ydl video <URL>`. Use the explicit subcommands for playlists, \
                  channels, batch files, config, or dependency management.",
    arg_required_else_help = true,
)]
pub struct Cli {
    /// Video URL to download (shortcut for `ydl video <URL>`).
    /// Ignored when a subcommand is given.
    pub url: Option<String>,

    /// Options applied to the default-video shortcut. Subcommands have their own copies.
    #[command(flatten)]
    pub opts: DownloadOpts,

    #[command(subcommand)]
    pub command: Option<Command>,

    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Download a single video
    Video {
        url: String,
        #[command(flatten)]
        opts: DownloadOpts,
    },
    /// Download every video in a playlist
    Playlist {
        url: String,
        #[command(flatten)]
        opts: DownloadOpts,
    },
    /// Download every video on a channel (URL or @handle)
    Channel {
        url: String,
        #[command(flatten)]
        opts: DownloadOpts,
    },
    /// Download URLs listed (one per line) in FILE
    Batch {
        file: PathBuf,
        #[command(flatten)]
        opts: DownloadOpts,
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

#[derive(Subcommand, Debug)]
pub enum DepsAction {
    /// Show installed versions and resolved binary paths
    Status,
    /// Install any missing binaries
    Install,
    /// Re-check latest releases and update if newer
    Update,
}

#[derive(Args, Debug, Clone, Default)]
pub struct DownloadOpts {
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

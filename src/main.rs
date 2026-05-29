mod cli_sink;

use clap::Parser;
use cli_sink::CliSink;
use ydl::cli::{Cli, Command, ConfigAction, DepsAction, DownloadOpts};
use ydl::error::{Context, Result};
use ydl::{config, deps, download, summary};

fn init_tracing(verbose: u8) {
    use tracing_subscriber::{fmt, EnvFilter};
    let default = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    fmt().with_env_filter(filter).with_target(false).init();
}

#[tokio::main]
async fn main() {
    if let Err(e) = real_main().await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

async fn real_main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match cli.command {
        None => match cli.url {
            Some(url) => run_download(vec![url], cli.opts, download::Mode::Single).await,
            // arg_required_else_help should prevent reaching here, but be safe.
            None => anyhow::bail!("a URL or subcommand is required (try `ydl --help`)"),
        },
        Some(Command::Playlist { url, opts }) => {
            run_download(vec![url], opts, download::Mode::Playlist).await
        }
        Some(Command::Channel { url, opts }) => {
            run_download(vec![url], opts, download::Mode::Playlist).await
        }
        Some(Command::Batch { file, opts }) => {
            let urls = download::read_batch_file(&file).await?;
            if urls.is_empty() {
                anyhow::bail!("batch file {} contains no URLs", file.display());
            }
            run_download(urls, opts, download::Mode::Batch).await
        }
        Some(Command::Config { action }) => handle_config(action).await,
        Some(Command::Deps { action }) => handle_deps(action).await,
    }
}

async fn run_download(urls: Vec<String>, opts: DownloadOpts, mode: download::Mode) -> Result<()> {
    let mut cfg = config::load_or_init().context("load config")?;
    config::merge_opts(&mut cfg, &opts);

    if opts.update {
        eprintln!("Updating yt-dlp + ffmpeg…");
        deps::update_all().await?;
    }

    let sink = CliSink::new(cfg.parallel.jobs.max(1));
    let summary = download::run_with_sink(&cfg, &opts, urls, mode, &sink).await?;
    let failures = summary.failure_count();
    sink.finish(failures);
    summary::render(&summary);

    if failures > 0 {
        anyhow::bail!("{failures} download(s) failed");
    }
    Ok(())
}

async fn handle_config(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Path => {
            println!("{}", config::config_path()?.display());
        }
        ConfigAction::Show => {
            let cfg = config::load_or_init()?;
            let s = toml::to_string_pretty(&cfg)?;
            println!("{s}");
        }
        ConfigAction::Init => {
            let path = config::config_path()?;
            config::write_default(&path)?;
            println!("wrote default config to {}", path.display());
        }
        ConfigAction::Edit => {
            let path = config::config_path()?;
            if !path.exists() {
                config::write_default(&path)?;
            }
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| {
                if cfg!(windows) { "notepad".into() } else { "vi".into() }
            });
            let status = tokio::process::Command::new(&editor)
                .arg(&path)
                .status()
                .await
                .with_context(|| format!("launch editor {editor}"))?;
            if !status.success() {
                anyhow::bail!("editor {editor} exited with {status}");
            }
        }
    }
    Ok(())
}

async fn handle_deps(action: DepsAction) -> Result<()> {
    match action {
        DepsAction::Status => {
            let cfg = config::load_or_init()?;
            deps::status(&cfg).await?;
        }
        DepsAction::Install => {
            let cfg = config::load_or_init()?;
            for tool in [deps::Tool::YtDlp, deps::Tool::Ffmpeg] {
                if deps::resolve(&cfg, tool)?.is_none() {
                    deps::install(tool).await?;
                } else {
                    eprintln!("{}: already installed", tool.label());
                }
            }
        }
        DepsAction::Update => {
            deps::update_all().await?;
        }
    }
    Ok(())
}

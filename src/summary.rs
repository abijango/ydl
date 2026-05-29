use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Duration;

pub struct Summary {
    pub directory: PathBuf,
    pub elapsed: Duration,
    pub dry_run: bool,
    pub kind: Kind,
}

pub enum Kind {
    Single {
        title: Option<String>,
        url: Option<String>,
        bytes: u64,
        skipped: bool,
        failed: Option<String>,
    },
    Multi {
        playlist_title: Option<String>,
        downloaded: usize,
        skipped: usize,
        failed: usize,
        total_bytes: u64,
    },
}

impl Summary {
    /// Number of items that failed in this run.
    pub fn failure_count(&self) -> usize {
        match &self.kind {
            Kind::Single { failed, .. } => usize::from(failed.is_some()),
            Kind::Multi { failed, .. } => *failed,
        }
    }

    /// A serializable, UI-friendly projection. The `banner` and `rows` reuse the
    /// exact same builders as the terminal renderer (sans ANSI color), so the
    /// desktop app shows identical text to the CLI summary table.
    pub fn to_dto(&self) -> SummaryDto {
        SummaryDto {
            directory: self.directory.display().to_string(),
            elapsed_ms: self.elapsed.as_millis() as u64,
            dry_run: self.dry_run,
            banner: build_banner(self, false),
            rows: build_rows(self)
                .into_iter()
                .map(|(label, value)| SummaryRow {
                    label: label.to_string(),
                    value,
                })
                .collect(),
            failures: self.failure_count(),
        }
    }
}

/// Serializable projection of a [`Summary`] for UIs.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryDto {
    pub directory: String,
    pub elapsed_ms: u64,
    pub dry_run: bool,
    pub banner: String,
    pub rows: Vec<SummaryRow>,
    pub failures: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SummaryRow {
    pub label: String,
    pub value: String,
}

const YELLOW: &str = "\x1b[33m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

fn use_color() -> bool {
    std::io::stderr().is_terminal()
}

pub fn render(s: &Summary) {
    let color = use_color();
    let banner = build_banner(s, color);
    let rows = build_rows(s);

    let label_w = rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);

    eprintln!();
    eprintln!("  {banner}");
    eprintln!();
    for (label, value) in rows {
        eprintln!("    {label:<width$}  {value}", width = label_w);
    }
    eprintln!();
}

fn build_banner(s: &Summary, color: bool) -> String {
    let (icon, text, col) = if s.dry_run {
        ("⚠", dry_run_text(s), YELLOW)
    } else if has_failures(s) {
        ("✗", failure_text(s), RED)
    } else {
        ("✔", success_text(s), GREEN)
    };
    if color {
        format!("{col}{icon} {text}{RESET}")
    } else {
        format!("{icon} {text}")
    }
}

fn build_rows(s: &Summary) -> Vec<(&'static str, String)> {
    let mut rows = Vec::new();
    match &s.kind {
        Kind::Single { title, url, bytes, skipped, failed } => {
            match (title, url) {
                (Some(t), _) => rows.push(("Title", t.clone())),
                (None, Some(u)) => rows.push(("URL", u.clone())),
                _ => {}
            }
            rows.push(("Directory", s.directory.display().to_string()));
            if s.dry_run {
                // size/time are meaningless for dry-run
            } else if *skipped {
                rows.push(("Status", "skipped (already in archive)".into()));
            } else if failed.is_none() {
                rows.push(("Size", human_bytes(*bytes)));
                rows.push(("Time", human_duration(s.elapsed)));
            }
            if let Some(e) = failed {
                rows.push(("Failed", truncate(e, 200)));
            }
        }
        Kind::Multi {
            playlist_title,
            downloaded,
            skipped,
            failed,
            total_bytes,
        } => {
            if let Some(t) = playlist_title {
                rows.push(("Playlist", t.clone()));
            }
            let mut parts = vec![format!("{downloaded} downloaded")];
            if *skipped > 0 {
                parts.push(format!("{skipped} skipped"));
            }
            rows.push(("Videos", parts.join(", ")));
            if *failed > 0 {
                rows.push(("Failed", failed.to_string()));
            }
            rows.push(("Directory", s.directory.display().to_string()));
            if !s.dry_run {
                rows.push(("Total size", human_bytes(*total_bytes)));
                rows.push(("Time", human_duration(s.elapsed)));
            }
        }
    }
    rows
}

fn has_failures(s: &Summary) -> bool {
    match &s.kind {
        Kind::Single { failed, .. } => failed.is_some(),
        Kind::Multi { failed, .. } => *failed > 0,
    }
}

fn success_text(s: &Summary) -> String {
    match &s.kind {
        Kind::Single { skipped: true, .. } => "Already downloaded (skipped)".into(),
        Kind::Single { .. } => "Download complete".into(),
        Kind::Multi {
            downloaded, skipped, ..
        } if *downloaded == 0 && *skipped > 0 => format!("All {skipped} videos already downloaded"),
        Kind::Multi { downloaded, .. } => {
            let plural = if *downloaded == 1 { "video" } else { "videos" };
            format!("Downloaded {downloaded} {plural}")
        }
    }
}

fn failure_text(s: &Summary) -> String {
    match &s.kind {
        Kind::Single { .. } => "Download failed".into(),
        Kind::Multi {
            downloaded, failed, ..
        } => format!("{downloaded} downloaded, {failed} failed"),
    }
}

fn dry_run_text(s: &Summary) -> String {
    match &s.kind {
        Kind::Single { .. } => "Dry run — 1 video would be downloaded".into(),
        Kind::Multi { downloaded, .. } => {
            let plural = if *downloaded == 1 { "video" } else { "videos" };
            format!("Dry run — {downloaded} {plural} would be downloaded")
        }
    }
}

fn human_bytes(b: u64) -> String {
    const K: f64 = 1024.0;
    let bf = b as f64;
    if bf < K {
        format!("{b} B")
    } else if bf < K * K {
        format!("{:.1} KB", bf / K)
    } else if bf < K * K * K {
        format!("{:.1} MB", bf / (K * K))
    } else {
        format!("{:.2} GB", bf / (K * K * K))
    }
}

fn human_duration(d: Duration) -> String {
    let total_ms = d.as_millis();
    let secs = (total_ms / 1000) as u64;
    if total_ms < 60_000 {
        format!("{:.1}s", total_ms as f64 / 1000.0)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_formatting() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KB");
        assert_eq!(human_bytes(15 * 1024 * 1024), "15.0 MB");
        assert_eq!(human_bytes(2 * 1024 * 1024 * 1024), "2.00 GB");
    }

    #[test]
    fn duration_formatting() {
        assert_eq!(human_duration(Duration::from_millis(8300)), "8.3s");
        assert_eq!(human_duration(Duration::from_secs(125)), "2m 5s");
        assert_eq!(human_duration(Duration::from_secs(3700)), "1h 1m");
    }
}

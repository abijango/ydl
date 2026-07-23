use crate::config::Config;
use std::path::{Path, PathBuf};

/// Resolve the archive path for a given output directory.
/// Relative archive paths resolve under the output dir; absolute paths are used as-is.
/// Returns None when archives are disabled.
pub fn path_for(cfg: &Config, output_dir: &Path) -> Option<PathBuf> {
    if !cfg.archive.enabled {
        return None;
    }
    let p = &cfg.archive.path;
    if p.is_absolute() {
        Some(p.clone())
    } else {
        Some(output_dir.join(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn disabled_returns_none() {
        let mut cfg = Config::default();
        cfg.archive.enabled = false;
        assert!(path_for(&cfg, Path::new("/out")).is_none());
    }

    #[test]
    fn relative_joins_output_dir() {
        let cfg = Config::default();
        let p = path_for(&cfg, Path::new("/out")).unwrap();
        assert_eq!(p, PathBuf::from("/out/.ydl-archive"));
    }

    #[test]
    fn absolute_used_as_is() {
        let mut cfg = Config::default();
        cfg.archive.path = PathBuf::from("/abs/archive.txt");
        let p = path_for(&cfg, Path::new("/out")).unwrap();
        assert_eq!(p, PathBuf::from("/abs/archive.txt"));
    }
}

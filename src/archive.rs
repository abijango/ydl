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

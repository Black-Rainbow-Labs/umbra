//! Writes the trajectory file. It is rewritten after every step, so a crash still leaves a
//! valid record of everything up to that point.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::models::trajectory::Trajectory;
use crate::utils::paths::umbra_home;

/// Picks the trajectory path: the configured one, or a new file under `~/.umbra/trajectories`
/// named by the start time.
/// @param configured - the `output` setting, possibly empty
pub fn trajectory_path(configured: &str) -> PathBuf {
    if !configured.is_empty() {
        return PathBuf::from(configured);
    }
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    umbra_home().join("trajectories").join(format!("{stamp}.traj.json"))
}

/// Writes the trajectory as indented JSON, through a temporary file so a reader never sees a
/// half written file.
/// @param path - where to write
/// @param trajectory - the record
pub fn save_trajectory(path: &Path, trajectory: &Trajectory) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_string_pretty(trajectory)?)?;
    std::fs::rename(&temp, path).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

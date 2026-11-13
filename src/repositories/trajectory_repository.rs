//! Writes the trajectory JSON file, in the mini-swe-agent-1.1 format. Umbra writes it when it is
//! asked for (`-o`, the `output` setting, `umbra export`); the session journal next to it is the
//! record written after every step.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::models::trajectory::Trajectory;
use crate::utils::paths::umbra_home;

/// Picks the trajectory path: the configured one, or `~/.umbra/trajectories/<session id>.traj.json`.
/// @param configured - the `output` setting, possibly empty
/// @param session_id - the session id
pub fn trajectory_path(configured: &str, session_id: &str) -> PathBuf {
    if !configured.is_empty() {
        return PathBuf::from(configured);
    }
    umbra_home().join("trajectories").join(format!("{session_id}.traj.json"))
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

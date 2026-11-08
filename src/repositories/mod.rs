//! Everything Umbra reads from or writes to disk.

pub mod command_repository;
pub mod config_repository;
pub mod instruction_repository;
pub mod mcp_config_repository;
pub mod skill_repository;
pub mod trajectory_repository;

use std::path::PathBuf;

use crate::models::config::SourcesConfig;

/// The folders the loaders search, and which harnesses' files they may read.
#[derive(Debug, Clone)]
pub struct DiscoveryContext {
    pub home: PathBuf,
    /// The folder Umbra runs commands in.
    pub cwd: PathBuf,
    /// The nearest folder above `cwd` that holds `.git`, or `cwd` itself.
    pub project_root: PathBuf,
    pub sources: SourcesConfig,
}

impl DiscoveryContext {
    /// Lists the folders from the project root down to `cwd`, root first.
    pub fn project_folders(&self) -> Vec<PathBuf> {
        crate::utils::paths::folders_from_root(&self.project_root, &self.cwd)
    }
}

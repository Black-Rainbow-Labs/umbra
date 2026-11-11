//! Turning an MCP server on or off for every session: the `mcp.only` and `mcp.disabled` lists in a
//! config file. The `/mcp` panel, the configuration modal and `umbra mcp enable` and
//! `umbra mcp disable` all change the lists through these functions, so they agree on what on and
//! off mean.

use std::path::Path;

use anyhow::Result;
use serde_json::Value;

use super::mcp::is_selected;
use super::settings_service::write_changes;
use crate::models::config::McpConfig;
use crate::repositories::config_repository::load_config;
use crate::utils::paths::{project_root, umbra_home};

/// The `only` and `disabled` lists after turning one server on or off. Off adds the name to
/// `disabled` and takes it out of `only`. On takes it out of `disabled` and, when `only` is not
/// empty, adds it to `only`, because a server outside a non empty `only` list never starts.
/// @param only - the `mcp.only` list
/// @param disabled - the `mcp.disabled` list
/// @param name - the server
/// @param on - true to turn it on
pub fn server_lists(only: &[String], disabled: &[String], name: &str, on: bool) -> (Vec<String>, Vec<String>) {
    let mut only: Vec<String> = only.to_vec();
    let mut disabled: Vec<String> = disabled.iter().filter(|d| *d != name).cloned().collect();
    if on {
        if !only.is_empty() && !only.iter().any(|o| o == name) {
            only.push(name.to_string());
        }
    } else {
        only.retain(|o| o != name);
        disabled.push(name.to_string());
    }
    (only, disabled)
}

/// The `mcp.only` and `mcp.disabled` lists one config file sets, empty when it sets none.
/// @param file - the config file, which may not exist
fn file_lists(file: &Path) -> (Vec<String>, Vec<String>) {
    let table: toml::Table = std::fs::read_to_string(file).ok().and_then(|t| t.parse().ok()).unwrap_or_default();
    let list = |key: &str| -> Vec<String> {
        table.get("mcp").and_then(|m| m.get(key)).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default()
    };
    (list("only"), list("disabled"))
}

/// The changes that turn a server on or off in one file, as setting paths and values for
/// `write_changes`. A list the file does not have and that stays empty is left out.
/// @param file - the config file
/// @param name - the server
/// @param on - true to turn it on
pub fn changes_for(file: &Path, name: &str, on: bool) -> Vec<(String, Option<Value>)> {
    let (only, disabled) = file_lists(file);
    let (new_only, new_disabled) = server_lists(&only, &disabled, name, on);
    let mut changes = Vec::new();
    for (key, before, after) in [("mcp.only", only, new_only), ("mcp.disabled", disabled, new_disabled)] {
        if before != after {
            changes.push((key.to_string(), Some(Value::from(after))));
        }
    }
    changes
}

/// Turns a server on or off for every session by changing a config file.
/// @param file - `~/.umbra/config.toml` or a project's `.umbra/config.toml`
/// @param name - the server
/// @param on - true to turn it on
pub fn set_saved(file: &Path, name: &str, on: bool) -> Result<()> {
    let changes = changes_for(file, name, on);
    if changes.is_empty() {
        return Ok(());
    }
    write_changes(file, &changes)
}

/// The user config file, `~/.umbra/config.toml`.
pub fn user_file() -> std::path::PathBuf {
    umbra_home().join("config.toml")
}

/// The project config file for a folder, `<project>/.umbra/config.toml`.
/// @param cwd - the working folder
pub fn project_file(cwd: &Path) -> std::path::PathBuf {
    project_root(cwd).join(".umbra").join("config.toml")
}

/// The MCP settings the config files give a folder, read again from disk, so a change written a
/// moment ago is seen.
/// @param cwd - the working folder
pub fn saved_settings(cwd: &Path) -> McpConfig {
    load_config(&project_root(cwd), &[]).map(|c| c.mcp).unwrap_or_default()
}

/// Whether the config files let a server start in every new session.
/// @param cwd - the working folder
/// @param name - the server
pub fn saved_enabled(cwd: &Path, name: &str) -> bool {
    is_selected(name, &saved_settings(cwd))
}

/// True when the project file turns a server off even though the user file lets it start, so a
/// change in the user file cannot turn it on.
/// @param cwd - the working folder
/// @param name - the server
pub fn project_turns_off(cwd: &Path, name: &str) -> bool {
    let (only, disabled) = file_lists(&project_file(cwd));
    disabled.iter().any(|d| d == name) || (!only.is_empty() && !only.iter().any(|o| o == name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn off_adds_to_disabled_and_leaves_only() {
        let (only, disabled) = server_lists(&names(&["a", "b"]), &names(&["c"]), "a", false);
        assert_eq!(only, names(&["b"]));
        assert_eq!(disabled, names(&["c", "a"]));
    }

    #[test]
    fn on_with_an_empty_only_list_only_leaves_disabled() {
        let (only, disabled) = server_lists(&[], &names(&["a", "c"]), "a", true);
        assert!(only.is_empty());
        assert_eq!(disabled, names(&["c"]));
    }

    #[test]
    fn on_with_an_only_list_joins_it() {
        let (only, disabled) = server_lists(&names(&["b"]), &[], "a", true);
        assert_eq!(only, names(&["b", "a"]));
        assert!(disabled.is_empty());
        let (only, _) = server_lists(&names(&["a"]), &[], "a", true);
        assert_eq!(only, names(&["a"]), "no second entry");
    }

    #[test]
    fn a_file_is_changed_and_its_other_keys_kept() {
        let dir = std::env::temp_dir().join(format!("umbra-mcp-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("config.toml");
        std::fs::write(&file, "# mine\nmodel = \"x/y\"\n").unwrap();
        set_saved(&file, "blender", false).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("# mine") && text.contains("disabled = [\"blender\"]"), "{text}");
        set_saved(&file, "blender", true).unwrap();
        assert_eq!(file_lists(&file), (Vec::new(), Vec::new()));
        std::fs::remove_file(&file).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}

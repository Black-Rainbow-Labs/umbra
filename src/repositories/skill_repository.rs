//! Finds Agent Skills: folders holding a SKILL.md whose frontmatter gives a name and description.
//! Project skills come first, searched from the working folder up to the project root, then user
//! skills. When two skills share a name the first one found wins, so a project skill overrides a
//! user skill of the same name.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::mcp_config_repository::codex_home;
use super::DiscoveryContext;
use crate::models::context::{Scope, Skill};
use crate::utils::frontmatter::{split_frontmatter, string_field};
use crate::utils::paths::{canonical_key, expand_tilde};

/// How deep under a skills folder Umbra looks for SKILL.md. Three levels reaches Claude Code's
/// synced claude.ai skills, which sit one folder deeper than ordinary skills.
const MAX_SKILL_DEPTH: usize = 3;

/// Finds every skill.
/// @param ctx - the folders and enabled sources
/// @param extra_paths - more skills folders from the config
/// @param disabled - skill names to leave out
pub fn discover_skills(ctx: &DiscoveryContext, extra_paths: &[String], disabled: &[String]) -> Vec<Skill> {
    let mut roots: Vec<(PathBuf, &'static str, Scope)> = extra_paths.iter().map(|p| (expand_tilde(p), "umbra", Scope::Project)).collect();
    roots.extend(project_roots(ctx));
    roots.extend(user_roots(ctx));
    let mut skills: Vec<Skill> = Vec::new();
    let mut seen_files = HashSet::new();
    for (root, source, scope) in roots {
        let mut found = Vec::new();
        find_skill_files(&root, 0, &mut found);
        found.sort();
        for file in found {
            if !seen_files.insert(canonical_key(&file)) {
                continue;
            }
            if let Some(skill) = read_skill(&file, source, scope) {
                if !disabled.contains(&skill.name) && !skills.iter().any(|s| s.name == skill.name) {
                    skills.push(skill);
                }
            }
        }
    }
    skills
}

/// Project skills folders, for each folder from the working folder up to the project root.
/// @param ctx - the folders and enabled sources
fn project_roots(ctx: &DiscoveryContext) -> Vec<(PathBuf, &'static str, Scope)> {
    let s = &ctx.sources;
    let mut folders = ctx.project_folders();
    folders.reverse();
    let mut roots = Vec::new();
    for folder in folders {
        let entries: [(bool, &str, &str, &'static str); 10] = [
            (s.umbra, ".umbra", "skills", "umbra"),
            (s.agents, ".agents", "skills", "agents"),
            (s.claude, ".claude", "skills", "claude"),
            (s.codex, ".codex", "skills", "codex"),
            (s.gemini, ".gemini", "skills", "gemini"),
            (s.opencode, ".opencode", "skills", "opencode"),
            (s.opencode, ".opencode", "skill", "opencode"),
            (s.copilot, ".github", "skills", "copilot"),
            (s.windsurf, ".windsurf", "skills", "windsurf"),
            (s.windsurf, ".devin", "skills", "windsurf"),
        ];
        for (enabled, dot, sub, source) in entries {
            if enabled {
                roots.push((folder.join(dot).join(sub), source, Scope::Project));
            }
        }
    }
    roots
}

/// User skills folders, including the skills of installed Claude Code plugins.
/// @param ctx - the folders and enabled sources
fn user_roots(ctx: &DiscoveryContext) -> Vec<(PathBuf, &'static str, Scope)> {
    let (h, s) = (&ctx.home, &ctx.sources);
    let mut entries: Vec<(bool, PathBuf, &'static str)> = vec![
        (s.umbra, h.join(".umbra").join("skills"), "umbra"),
        (s.agents, h.join(".agents").join("skills"), "agents"),
        (s.claude, h.join(".claude").join("skills"), "claude"),
    ];
    if s.claude {
        entries.extend(claude_plugin_skill_dirs(h).into_iter().map(|p| (true, p, "claude_plugin")));
    }
    entries.extend([
        (s.codex, codex_home(h).join("skills"), "codex"),
        (s.gemini, h.join(".gemini").join("skills"), "gemini"),
        (s.opencode, h.join(".config").join("opencode").join("skills"), "opencode"),
        (s.opencode, h.join(".config").join("opencode").join("skill"), "opencode"),
        (s.copilot, h.join(".copilot").join("skills"), "copilot"),
        (s.windsurf, h.join(".config").join("devin").join("skills"), "windsurf"),
    ]);
    entries.into_iter().filter(|e| e.0).map(|(_, p, src)| (p, src, Scope::User)).collect()
}

/// Reads `~/.claude/plugins/installed_plugins.json` and returns each installed plugin's
/// `skills` folder. Plugins in the marketplace checkout that are not installed are ignored.
/// @param home - the user's home folder
pub fn claude_plugin_skill_dirs(home: &Path) -> Vec<PathBuf> {
    let file = home.join(".claude").join("plugins").join("installed_plugins.json");
    let Ok(text) = std::fs::read_to_string(file) else { return Vec::new() };
    let Ok(json) = serde_json::from_str::<Value>(&text) else { return Vec::new() };
    let mut dirs = Vec::new();
    if let Some(plugins) = json["plugins"].as_object() {
        for installs in plugins.values() {
            for install in installs.as_array().cloned().unwrap_or_default() {
                if let Some(path) = install["installPath"].as_str() {
                    dirs.push(PathBuf::from(path).join("skills"));
                }
            }
        }
    }
    dirs
}

/// Walks a skills folder for SKILL.md files, skipping hidden folders and node_modules.
/// @param dir - the folder
/// @param depth - how deep the walk is
/// @param out - the files found
fn find_skill_files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if !path.is_dir() || name.starts_with('.') || name == "node_modules" {
            continue;
        }
        let skill_file = path.join("SKILL.md");
        if skill_file.is_file() {
            out.push(skill_file);
        } else if depth + 1 < MAX_SKILL_DEPTH {
            find_skill_files(&path, depth + 1, out);
        }
    }
}

/// Reads a SKILL.md's frontmatter. The name falls back to the folder name. A skill without a
/// description, or one that asks not to be offered to the model, is left out.
/// @param file - the SKILL.md path
/// @param source - which harness's folder it is in
/// @param scope - user or project
fn read_skill(file: &Path, source: &str, scope: Scope) -> Option<Skill> {
    let text = std::fs::read_to_string(file).ok()?;
    let (front, _) = split_frontmatter(&text);
    if front.get("disable-model-invocation").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let folder = file.parent()?.file_name()?.to_string_lossy().to_string();
    let name = Some(string_field(&front, "name")).filter(|n| !n.is_empty()).unwrap_or(folder);
    let mut description = string_field(&front, "description");
    let when = string_field(&front, "when_to_use");
    if !when.is_empty() {
        description = format!("{description} {when}");
    }
    if description.is_empty() {
        return None;
    }
    Some(Skill { name, description, path: file.to_path_buf(), source: source.to_string(), scope })
}

/// Reads a skill's body without its frontmatter, for running a skill as a slash command.
/// @param skill - the skill
pub fn skill_body(skill: &Skill) -> String {
    std::fs::read_to_string(&skill.path).map(|t| split_frontmatter(&t).1).unwrap_or_default()
}

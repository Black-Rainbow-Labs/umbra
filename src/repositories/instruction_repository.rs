//! Finds instruction files (CLAUDE.md, AGENTS.md, GEMINI.md, Cursor and Copilot rules, and the
//! rest) and returns them in the order they go into the system prompt: user files first, then
//! project files from the project root down to the working folder, so the most specific file is
//! read last.

use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use regex::Regex;
use serde_json::Value;

use super::mcp_config_repository::codex_home;
use super::DiscoveryContext;
use crate::models::context::{InstructionFile, Scope};
use crate::utils::frontmatter::split_frontmatter;
use crate::utils::paths::{canonical_key, expand_tilde};

/// Claude Code follows `@path` imports up to four hops deep.
const MAX_IMPORT_DEPTH: usize = 4;

/// How a file decides whether it applies to every request.
#[derive(Clone, Copy, PartialEq)]
enum Rule {
    /// Always applies.
    Always,
    /// A rule file with frontmatter: only rules meant for every file apply.
    Conditional,
}

/// A file or folder of files to look for.
struct Candidate {
    path: PathBuf,
    source: &'static str,
    scope: Scope,
    rule: Rule,
    /// Whether `@path` imports are followed (Claude Code and Gemini CLI files).
    imports: bool,
}

/// Loads every instruction file, de duplicated by path and by content, within the byte budget.
/// @param ctx - the folders and enabled sources
/// @param max_bytes - stop adding files after this many bytes
/// @param extra_files - files from the config that always load
pub fn load_instructions(ctx: &DiscoveryContext, max_bytes: usize, extra_files: &[String]) -> Vec<InstructionFile> {
    let mut seen_paths = HashSet::new();
    let mut seen_content = HashSet::new();
    let mut files = Vec::new();
    let mut total = 0;
    for candidate in candidates(ctx, extra_files) {
        for path in expand_candidate(&candidate.path) {
            let Some(file) = read_instruction(&path, &candidate, ctx, &mut seen_paths) else { continue };
            if !seen_content.insert(content_hash(&file.content)) {
                continue;
            }
            if total + file.content.len() > max_bytes {
                eprintln!("umbra: skipped {} because the instruction budget of {max_bytes} bytes is used up", path.display());
                continue;
            }
            total += file.content.len();
            files.push(file);
        }
    }
    files
}

/// Builds the candidate list in prompt order for the enabled sources.
/// @param ctx - the folders and enabled sources
/// @param extra_files - files from the config
fn candidates(ctx: &DiscoveryContext, extra_files: &[String]) -> Vec<Candidate> {
    let mut list = user_candidates(ctx);
    list.extend(project_root_candidates(ctx));
    for folder in ctx.project_folders() {
        list.extend(folder_candidates(ctx, &folder));
    }
    for extra in extra_files {
        list.push(Candidate { path: expand_tilde(extra), source: "umbra", scope: Scope::Project, rule: Rule::Always, imports: true });
    }
    list
}

/// User level files: `~/.claude/CLAUDE.md`, `~/.codex/AGENTS.md`, `~/.gemini/GEMINI.md`, and the rest.
/// @param ctx - the folders and enabled sources
fn user_candidates(ctx: &DiscoveryContext) -> Vec<Candidate> {
    let (h, s) = (&ctx.home, &ctx.sources);
    let codex = codex_home(h);
    let codex_file = if non_empty(&codex.join("AGENTS.override.md")) { codex.join("AGENTS.override.md") } else { codex.join("AGENTS.md") };
    let entries: Vec<(bool, PathBuf, &'static str, Rule, bool)> = vec![
        (s.umbra, h.join(".umbra").join("UMBRA.md"), "umbra", Rule::Always, true),
        (s.claude, h.join(".claude").join("CLAUDE.md"), "claude", Rule::Always, true),
        (s.claude, h.join(".claude").join("rules"), "claude", Rule::Conditional, true),
        (s.codex, codex_file, "codex", Rule::Always, false),
        (s.gemini, h.join(".gemini").join("GEMINI.md"), "gemini", Rule::Always, true),
        (s.opencode, h.join(".config").join("opencode").join("AGENTS.md"), "opencode", Rule::Always, false),
        (s.agents, h.join(".agents").join("AGENTS.md"), "agents", Rule::Always, false),
        (s.windsurf, h.join(".devin").join("global_rules.md"), "windsurf", Rule::Always, false),
        (s.windsurf, h.join(".codeium").join("windsurf").join("memories").join("global_rules.md"), "windsurf", Rule::Always, false),
        (s.cline, h.join("Documents").join("Cline").join("Rules"), "cline", Rule::Conditional, false),
        (s.cline, h.join(".cline").join("rules"), "cline", Rule::Conditional, false),
    ];
    to_candidates(entries, Scope::User)
}

/// Files read once from the project root: rule folders and single file rules.
/// @param ctx - the folders and enabled sources
fn project_root_candidates(ctx: &DiscoveryContext) -> Vec<Candidate> {
    let (r, s) = (&ctx.project_root, &ctx.sources);
    let entries: Vec<(bool, PathBuf, &'static str, Rule, bool)> = vec![
        (s.claude, r.join(".claude").join("rules"), "claude", Rule::Conditional, true),
        (s.cursor, r.join(".cursorrules"), "cursor", Rule::Always, false),
        (s.cursor, r.join(".cursor").join("rules"), "cursor", Rule::Conditional, false),
        (s.copilot, r.join(".github").join("copilot-instructions.md"), "copilot", Rule::Always, false),
        (s.copilot, r.join(".github").join("instructions"), "copilot", Rule::Conditional, false),
        (s.windsurf, r.join(".windsurfrules"), "windsurf", Rule::Always, false),
        (s.windsurf, r.join(".windsurf").join("rules"), "windsurf", Rule::Conditional, false),
        (s.windsurf, r.join(".devin").join("rules"), "windsurf", Rule::Conditional, false),
        (s.cline, r.join(".clinerules"), "cline", Rule::Conditional, false),
        (s.cline, r.join(".cline").join("rules"), "cline", Rule::Conditional, false),
    ];
    to_candidates(entries, Scope::Project)
}

/// Files read in every folder from the project root down to the working folder. Codex reads one
/// AGENTS file per folder: `AGENTS.override.md` when it has content, otherwise `AGENTS.md`.
/// @param ctx - the folders and enabled sources
/// @param folder - one folder on the way down
fn folder_candidates(ctx: &DiscoveryContext, folder: &Path) -> Vec<Candidate> {
    let s = &ctx.sources;
    let agents = if non_empty(&folder.join("AGENTS.override.md")) { folder.join("AGENTS.override.md") } else { folder.join("AGENTS.md") };
    let reads_agents = s.agents || s.codex || s.opencode || s.cursor || s.copilot || s.windsurf || s.cline;
    let entries: Vec<(bool, PathBuf, &'static str, Rule, bool)> = vec![
        (reads_agents, agents, "agents", Rule::Always, false),
        (s.claude, folder.join("CLAUDE.md"), "claude", Rule::Always, true),
        (s.claude, folder.join(".claude").join("CLAUDE.md"), "claude", Rule::Always, true),
        (s.claude, folder.join("CLAUDE.local.md"), "claude", Rule::Always, true),
        (s.gemini, folder.join("GEMINI.md"), "gemini", Rule::Always, true),
        (s.umbra, folder.join("UMBRA.md"), "umbra", Rule::Always, true),
    ];
    to_candidates(entries, Scope::Project)
}

/// Turns a table of entries into candidates, dropping the disabled ones.
/// @param entries - enabled flag, path, source, rule kind, imports flag
/// @param scope - user or project
fn to_candidates(entries: Vec<(bool, PathBuf, &'static str, Rule, bool)>, scope: Scope) -> Vec<Candidate> {
    entries.into_iter().filter(|e| e.0).map(|(_, path, source, rule, imports)| Candidate { path, source, scope, rule, imports }).collect()
}

/// A file is itself; a folder is every `.md`, `.mdc` and `.txt` file under it, in name order.
/// @param path - a file or folder
fn expand_candidate(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    if !path.is_dir() {
        return Vec::new();
    }
    let mut files = Vec::new();
    collect_rule_files(path, &mut files, 0);
    files.sort();
    files
}

/// Walks a rules folder, at most four levels deep.
/// @param dir - the folder
/// @param out - the files found
/// @param depth - how deep the walk is
fn collect_rule_files(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && depth < 4 {
            collect_rule_files(&path, out, depth + 1);
        } else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| ["md", "mdc", "txt"].contains(&e)) {
            out.push(path);
        }
    }
}

/// Reads one file, applies its rule conditions and expands its imports.
/// @param path - the file
/// @param candidate - where it came from
/// @param ctx - the folders, for `~` in imports
/// @param seen_paths - files already loaded
fn read_instruction(path: &Path, candidate: &Candidate, ctx: &DiscoveryContext, seen_paths: &mut HashSet<String>) -> Option<InstructionFile> {
    if !seen_paths.insert(canonical_key(path)) {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let body = if candidate.rule == Rule::Conditional { applies_always(&text)? } else { text };
    let mut content = body.trim().to_string();
    if content.is_empty() {
        return None;
    }
    if candidate.imports {
        let base = path.parent().unwrap_or(Path::new("."));
        content.push_str(&expand_imports(&content, base, ctx, seen_paths, 1));
    }
    Some(InstructionFile { path: path.to_path_buf(), source: candidate.source.to_string(), scope: candidate.scope, content })
}

/// Decides whether a rule file applies to every request, and returns its body if so. A file
/// with no frontmatter applies. With frontmatter, it applies when it says so: Cursor's
/// `alwaysApply: true`, Windsurf's `trigger: always_on`, Copilot's `applyTo: "**"`, or a Claude
/// Code or Cline rule with no `paths`. Rules tied to particular files are left out, because
/// Umbra does not track which files the agent is working on.
/// @param text - the whole rule file
fn applies_always(text: &str) -> Option<String> {
    let (front, body) = split_frontmatter(text);
    let map = front.as_object()?;
    if map.is_empty() {
        return Some(body);
    }
    let always = map.get("alwaysApply").and_then(Value::as_bool);
    let trigger = map.get("trigger").and_then(Value::as_str);
    let apply_to = map.get("applyTo").and_then(Value::as_str).map(|s| s.trim());
    let applies = always == Some(true) || trigger == Some("always_on") || matches!(apply_to, Some("**") | Some("**/*")) || (always.is_none() && trigger.is_none() && apply_to.is_none() && !map.contains_key("paths") && !map.contains_key("globs"));
    applies.then_some(body)
}

/// Finds `@path` imports in a file and returns the imported files' text, each with a heading.
/// Imports inside code blocks and inline code are ignored, and each file is imported once.
/// @param text - the file text
/// @param base - the folder relative imports resolve against
/// @param ctx - the folders, for `~`
/// @param seen_paths - files already loaded
/// @param depth - how many imports deep this file is
fn expand_imports(text: &str, base: &Path, ctx: &DiscoveryContext, seen_paths: &mut HashSet<String>, depth: usize) -> String {
    if depth > MAX_IMPORT_DEPTH {
        return String::new();
    }
    let mut out = String::new();
    for target in import_targets(text) {
        let path = resolve_import(&target, base, ctx);
        if !path.is_file() || !seen_paths.insert(canonical_key(&path)) {
            continue;
        }
        let Ok(imported) = std::fs::read_to_string(&path) else { continue };
        out.push_str(&format!("\n\n<!-- imported from {} -->\n{}", path.display(), imported.trim()));
        out.push_str(&expand_imports(&imported, path.parent().unwrap_or(base), ctx, seen_paths, depth + 1));
    }
    out
}

/// Lists the `@path` references in text outside code. A reference must start a line or follow
/// a space, which keeps email addresses out.
/// @param text - the file text
pub fn import_targets(text: &str) -> Vec<String> {
    let pattern = Regex::new(r"(?:^|\s)@((?:~|\.{1,2})?[/\\]?[A-Za-z0-9_.\-/\\:]+)").expect("valid regex");
    let inline_code = Regex::new(r"`[^`]*`").expect("valid regex");
    let mut targets = Vec::new();
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let line = inline_code.replace_all(line, "");
        for caps in pattern.captures_iter(&line) {
            let target = caps[1].trim_end_matches(['.', ',', ';', ':', ')']).to_string();
            if target.contains('.') || target.contains('/') {
                targets.push(target);
            }
        }
    }
    targets
}

/// Resolves an import path: `~/` is the home folder, an absolute path is itself, and anything
/// else is relative to the importing file.
/// @param target - the path after `@`
/// @param base - the importing file's folder
/// @param ctx - the folders
fn resolve_import(target: &str, base: &Path, ctx: &DiscoveryContext) -> PathBuf {
    if let Some(rest) = target.strip_prefix("~/") {
        return ctx.home.join(rest);
    }
    let path = PathBuf::from(target);
    if path.is_absolute() { path } else { base.join(path) }
}

/// Returns true when a file exists and has something other than whitespace in it.
/// @param path - the file
fn non_empty(path: &Path) -> bool {
    std::fs::read_to_string(path).map(|t| !t.trim().is_empty()).unwrap_or(false)
}

/// Hashes trimmed content, so two copies of one file (a hard link, a copy) load once.
/// @param content - the file text
fn content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.trim().hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_imports_outside_code_only() {
        let text = "See @docs/guide.md and @~/notes.md.\nmail me at a@b.com\n`@not/this.md`\n```\n@nor/this.md\n```";
        assert_eq!(import_targets(text), vec!["docs/guide.md".to_string(), "~/notes.md".to_string()]);
    }

    #[test]
    fn keeps_rules_meant_for_every_file() {
        assert!(applies_always("no frontmatter").is_some());
        assert!(applies_always("---\nalwaysApply: true\n---\nbody").is_some());
        assert!(applies_always("---\nalwaysApply: false\nglobs: \"*.ts\"\n---\nbody").is_none());
        assert!(applies_always("---\npaths:\n  - src/**\n---\nbody").is_none());
        assert!(applies_always("---\napplyTo: \"**\"\n---\nbody").is_some());
    }
}

//! Finds custom slash commands: Claude Code's `.claude/commands/*.md`, opencode's
//! `command/*.md`, Codex's `~/.codex/prompts/*.md` and Gemini CLI's `commands/*.toml`. A task
//! that starts with `/name` is replaced by that command's prompt.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::mcp_config_repository::codex_home;
use super::DiscoveryContext;
use crate::models::context::SlashCommand;
use crate::utils::frontmatter::{split_frontmatter, string_field};

/// Finds every command. Project commands come before user commands, and the first command with
/// a name wins. Subfolders become namespaces: `git/commit.md` is `/git:commit`.
/// @param ctx - the folders and enabled sources
pub fn discover_commands(ctx: &DiscoveryContext) -> Vec<SlashCommand> {
    let (r, h, s) = (&ctx.project_root, &ctx.home, &ctx.sources);
    let roots: Vec<(bool, PathBuf, &'static str)> = vec![
        (s.umbra, r.join(".umbra").join("commands"), "umbra"),
        (s.claude, r.join(".claude").join("commands"), "claude"),
        (s.gemini, r.join(".gemini").join("commands"), "gemini"),
        (s.opencode, r.join(".opencode").join("command"), "opencode"),
        (s.opencode, r.join(".opencode").join("commands"), "opencode"),
        (s.copilot, r.join(".github").join("prompts"), "copilot"),
        (s.umbra, h.join(".umbra").join("commands"), "umbra"),
        (s.claude, h.join(".claude").join("commands"), "claude"),
        (s.codex, codex_home(h).join("prompts"), "codex"),
        (s.gemini, h.join(".gemini").join("commands"), "gemini"),
        (s.opencode, h.join(".config").join("opencode").join("command"), "opencode"),
        (s.opencode, h.join(".config").join("opencode").join("commands"), "opencode"),
    ];
    let mut commands: Vec<SlashCommand> = Vec::new();
    for (_, root, source) in roots.into_iter().filter(|r| r.0) {
        let mut files = Vec::new();
        collect_command_files(&root, &root, &mut files, 0);
        files.sort();
        for (name, path) in files {
            if commands.iter().any(|c| c.name == name) {
                continue;
            }
            if let Some(command) = read_command(&name, &path, source) {
                commands.push(command);
            }
        }
    }
    commands
}

/// Walks a commands folder for `.md` and `.toml` files and names each by its path.
/// @param root - the commands folder
/// @param dir - the folder being walked
/// @param out - name and path pairs
/// @param depth - how deep the walk is
fn collect_command_files(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>, depth: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && depth < 3 {
            collect_command_files(root, &path, out, depth + 1);
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "md" && ext != "toml" {
            continue;
        }
        let relative = path.strip_prefix(root).unwrap_or(&path).with_extension("");
        let name = relative.to_string_lossy().replace(['\\', '/'], ":").trim_end_matches(".prompt").to_string();
        out.push((name, path));
    }
}

/// Reads one command file. Markdown files are a prompt with optional frontmatter; TOML files
/// hold `prompt` and `description` keys.
/// @param name - the command name
/// @param path - the file
/// @param source - which harness's folder it is in
fn read_command(name: &str, path: &Path, source: &str) -> Option<SlashCommand> {
    let text = std::fs::read_to_string(path).ok()?;
    let (description, body) = if path.extension().and_then(|e| e.to_str()) == Some("toml") {
        let value: Value = toml::from_str::<toml::Value>(&text).ok().and_then(|v| serde_json::to_value(v).ok())?;
        (string_field(&value, "description"), value["prompt"].as_str()?.to_string())
    } else {
        let (front, body) = split_frontmatter(&text);
        (string_field(&front, "description"), body)
    };
    Some(SlashCommand { name: name.to_string(), description, body, path: path.to_path_buf(), source: source.to_string() })
}

/// Fills a command's placeholders with the arguments: `$ARGUMENTS`, `{{args}}`, `$ARGUMENTS[N]`
/// and `$1` to `$9`. When the prompt has no placeholder, the arguments are added at the end.
/// @param body - the command prompt
/// @param arguments - the text after the command name
pub fn render_command(body: &str, arguments: &str) -> String {
    let words: Vec<&str> = arguments.split_whitespace().collect();
    let mut out = body.to_string();
    for i in (0..9).rev() {
        let word = words.get(i).copied().unwrap_or("");
        out = out.replace(&format!("$ARGUMENTS[{i}]"), word).replace(&format!("${}", i + 1), word);
    }
    out = out.replace("$ARGUMENTS", arguments).replace("{{args}}", arguments);
    if out == body && !arguments.is_empty() {
        out = format!("{body}\n\nARGUMENTS: {arguments}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_placeholders_or_appends_arguments() {
        assert_eq!(render_command("Fix issue $ARGUMENTS now", "42"), "Fix issue 42 now");
        assert_eq!(render_command("first=$1 second=$2", "a b"), "first=a second=b");
        assert_eq!(render_command("Do the thing", "extra"), "Do the thing\n\nARGUMENTS: extra");
        assert_eq!(render_command("{{args}}!", "hi"), "hi!");
    }
}

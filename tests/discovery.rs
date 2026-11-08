//! Builds a fake home folder and project in the layouts Claude Code, Codex, opencode and Cursor
//! use, and checks that Umbra finds the right instruction files, skills, commands and MCP servers.

mod common;

use common::{run_umbra, slash, text, Scratch};
use serde_json::{json, Value};

/// Lays out the fake home folder and project, and returns the working folder.
/// @param s - the test folder
fn build_fixture(s: &Scratch) -> std::path::PathBuf {
    let shared = "Shared user instructions.";
    s.write("home/.claude/CLAUDE.md", shared);
    s.write("home/.codex/AGENTS.md", shared);
    s.write("home/.agents/skills/alpha/SKILL.md", "---\nname: alpha\ndescription: Alpha from .agents\n---\nbody");
    s.write("home/.claude/skills/alpha/SKILL.md", "---\nname: alpha\ndescription: Alpha from .claude\n---\nbody");
    s.write("home/.claude/skills/hidden/SKILL.md", "---\nname: hidden\ndescription: x\ndisable-model-invocation: true\n---\n");
    s.write("home/.claude/skills/synced/abc_def/gamma/SKILL.md", "---\nname: gamma\ndescription: Synced claude.ai skill\n---\n");
    let plugin = s.path("plugin-install");
    s.write("plugin-install/skills/beta/SKILL.md", "---\ndescription: Beta from a plugin\n---\n");
    s.write("home/.claude/plugins/installed_plugins.json", &json!({ "version": 2, "plugins": { "p@m": [{ "installPath": slash(&plugin) }] } }).to_string());
    s.write("home/.config/opencode/opencode.json", &json!({ "mcp": { "oc": { "type": "local", "command": ["node", "oc.js"], "enabled": true }, "off": { "type": "local", "command": ["x"], "enabled": false } } }).to_string());
    s.write("home/.codex/config.toml", "[mcp_servers.cx]\ncommand = \"codex-server\"\nargs = [\"--flag\"]\n");
    std::fs::create_dir_all(s.path("project/.git")).unwrap();
    s.write("project/AGENTS.md", "Root agents file.");
    s.write("project/CLAUDE.md", "Root claude file. See @docs/extra.md for more.");
    s.write("project/docs/extra.md", "Imported extra rules.");
    s.write("project/.cursor/rules/always.mdc", "---\nalwaysApply: true\n---\nCursor always rule.");
    s.write("project/.cursor/rules/scoped.mdc", "---\nalwaysApply: false\nglobs: \"*.ts\"\n---\nCursor scoped rule.");
    s.write("project/.claude/commands/hi.md", "---\ndescription: Say hi\n---\nSay hi to $ARGUMENTS");
    s.write("project/.mcp.json", &json!({ "mcpServers": { "proj": { "command": "proj-server", "env": { "TOKEN": "${UMBRA_TEST_TOKEN:-fallback}" } } } }).to_string());
    s.write("project/sub/AGENTS.md", "Sub folder agents file.");
    let project = s.path("project");
    s.write("home/.claude.json", &json!({ "mcpServers": { "usr": { "type": "stdio", "command": "user-server" }, "proj": { "command": "loses-to-project" } }, "projects": { slash(&project): { "mcpServers": { "local": { "type": "http", "url": "http://localhost:1/mcp" } } } } }).to_string());
    s.path("project/sub")
}

/// Runs a listing subcommand with `--json` and parses the output.
/// @param s - the test folder
/// @param command - instructions, skills, commands or mcp
/// @param cwd - the working folder
fn list(s: &Scratch, command: &str, cwd: &std::path::Path) -> Value {
    let output = run_umbra(&s.path("home"), &[command, "--json", "--cwd", &slash(cwd)]);
    assert!(output.status.success(), "{}", text(&output));
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| panic!("not JSON: {}", text(&output)))
}

#[test]
fn loads_instruction_files_in_order_without_duplicates() {
    let s = Scratch::new("instructions");
    let cwd = build_fixture(&s);
    let files = list(&s, "instructions", &cwd);
    let contents: Vec<&str> = files.as_array().unwrap().iter().map(|f| f["content"].as_str().unwrap()).collect();
    assert_eq!(contents.iter().filter(|c| c.contains("Shared user instructions.")).count(), 1, "hard linked or copied files load once");
    let order = |needle: &str| contents.iter().position(|c| c.contains(needle)).unwrap_or_else(|| panic!("missing {needle}"));
    assert!(order("Shared user") < order("Cursor always rule.") && order("Cursor always rule.") < order("Root agents file.") && order("Root agents file.") < order("Sub folder agents file."));
    assert!(contents.iter().any(|c| c.contains("Root claude file.") && c.contains("Imported extra rules.")), "the @ import is expanded");
    assert!(!contents.iter().any(|c| c.contains("Cursor scoped rule.")), "rules tied to file globs are left out");
}

#[test]
fn finds_skills_from_every_location_with_project_and_order_precedence() {
    let s = Scratch::new("skills");
    let cwd = build_fixture(&s);
    let skills = list(&s, "skills", &cwd);
    let by_name = |name: &str| skills.as_array().unwrap().iter().find(|k| k["name"] == name).cloned();
    assert_eq!(by_name("alpha").unwrap()["description"], "Alpha from .agents");
    assert_eq!(by_name("beta").unwrap()["source"], "claude_plugin");
    assert!(by_name("gamma").is_some(), "synced claude.ai skills one folder deeper are found");
    assert!(by_name("hidden").is_none(), "disable-model-invocation hides a skill");
}

#[test]
fn finds_commands() {
    let s = Scratch::new("commands");
    let cwd = build_fixture(&s);
    let commands = list(&s, "commands", &cwd);
    assert_eq!(commands[0]["name"], "hi");
    assert!(commands[0]["body"].as_str().unwrap().contains("$ARGUMENTS"));
}

#[test]
fn reads_mcp_servers_from_every_format_with_precedence() {
    let s = Scratch::new("mcp");
    let cwd = build_fixture(&s);
    let servers = list(&s, "mcp", &cwd);
    let names: Vec<&str> = servers.as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap()).collect();
    for expected in ["proj", "local", "usr", "cx", "oc"] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }
    assert!(!names.contains(&"off"), "disabled opencode servers are skipped");
    let proj = servers.as_array().unwrap().iter().find(|v| v["name"] == "proj").unwrap();
    assert!(proj["transport"].as_str().unwrap().contains("proj-server"), "the project .mcp.json wins over ~/.claude.json");
    let local = servers.as_array().unwrap().iter().find(|v| v["name"] == "local").unwrap();
    assert!(local["transport"].as_str().unwrap().starts_with("http:"));
}

#[test]
fn a_disabled_source_is_not_read() {
    let s = Scratch::new("nosource");
    let cwd = build_fixture(&s);
    let output = run_umbra(&s.path("home"), &["skills", "--json", "--no-source", "agents", "--cwd", &slash(&cwd)]);
    let skills: Value = serde_json::from_slice(&output.stdout).unwrap();
    let alpha = skills.as_array().unwrap().iter().find(|k| k["name"] == "alpha").unwrap();
    assert_eq!(alpha["description"], "Alpha from .claude");
}

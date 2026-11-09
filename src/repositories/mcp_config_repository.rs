//! Finds MCP server definitions in every harness's config files and reads them into one shape.
//!
//! Precedence, highest first: Umbra's own config, then project files (`.mcp.json`, the Claude Code
//! entry for this project in `~/.claude.json`, `.cursor/mcp.json`, `.vscode/mcp.json`,
//! `.gemini/settings.json`, `opencode.json`, `.codex/config.toml`), then user files
//! (`~/.claude.json`, Claude Desktop, `~/.codex/config.toml`, `~/.gemini/settings.json`,
//! `~/.cursor/mcp.json`, `~/.config/opencode/opencode.json`). When a name appears twice, the first
//! one found wins and the others are ignored, the way Claude Code does it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::DiscoveryContext;
use crate::models::context::{McpServerDefinition, McpTransport, Scope};
use crate::utils::env_expand::expand_env;
use crate::utils::paths::app_config_dir;

/// Which file format a config file uses.
#[derive(Clone, Copy)]
enum Format {
    /// `{"mcpServers": {...}}`: Claude Code, Claude Desktop, Cursor, Gemini CLI.
    McpServers,
    /// `{"servers": {...}}`: VS Code.
    Servers,
    /// `{"mcp": {...}}`: opencode, old and new layouts.
    Opencode,
    /// `[mcp_servers.<name>]`: Codex.
    CodexToml,
    /// `~/.claude.json`: user servers plus `projects[<path>].mcpServers`.
    ClaudeJson,
}

/// One config file to read.
struct Candidate {
    path: PathBuf,
    format: Format,
    source: &'static str,
    scope: Scope,
}

/// Reads every MCP server definition, de duplicated by name.
/// @param ctx - the folders and enabled sources
/// @param umbra_servers - servers from Umbra's own config, which win over everything else
pub fn discover_mcp_servers(ctx: &DiscoveryContext, umbra_servers: &BTreeMap<String, Value>) -> Vec<McpServerDefinition> {
    let mut found: Vec<McpServerDefinition> = Vec::new();
    let umbra_origin = ctx.home.join(".umbra").join("config.toml");
    for (name, entry) in umbra_servers {
        push_unique(&mut found, parse_entry(name, entry, &umbra_origin, "umbra", Scope::User, ctx));
    }
    for candidate in candidates(ctx) {
        for definition in read_candidate(&candidate, ctx) {
            push_unique(&mut found, Some(definition));
        }
    }
    found
}

/// Adds a definition unless a server with that name is already there.
/// @param found - the definitions so far
/// @param definition - the next one, if it parsed
fn push_unique(found: &mut Vec<McpServerDefinition>, definition: Option<McpServerDefinition>) {
    if let Some(definition) = definition {
        if !found.iter().any(|f| f.name == definition.name) {
            found.push(definition);
        }
    }
}

/// Lists the config files to read, in precedence order, for the enabled sources.
/// @param ctx - the folders and enabled sources
fn candidates(ctx: &DiscoveryContext) -> Vec<Candidate> {
    let (root, home, s) = (&ctx.project_root, &ctx.home, &ctx.sources);
    let mut list = Vec::new();
    let mut add = |enabled: bool, path: PathBuf, format: Format, source: &'static str, scope: Scope| {
        if enabled {
            list.push(Candidate { path, format, source, scope });
        }
    };
    add(s.umbra, root.join(".umbra").join("mcp.json"), Format::McpServers, "umbra", Scope::Project);
    add(s.claude, root.join(".mcp.json"), Format::McpServers, "claude", Scope::Project);
    add(s.claude, home.join(".claude.json"), Format::ClaudeJson, "claude", Scope::Project);
    add(s.cursor, root.join(".cursor").join("mcp.json"), Format::McpServers, "cursor", Scope::Project);
    add(s.vscode, root.join(".vscode").join("mcp.json"), Format::Servers, "vscode", Scope::Project);
    add(s.gemini, root.join(".gemini").join("settings.json"), Format::McpServers, "gemini", Scope::Project);
    add(s.opencode, root.join("opencode.json"), Format::Opencode, "opencode", Scope::Project);
    add(s.opencode, root.join("opencode.jsonc"), Format::Opencode, "opencode", Scope::Project);
    add(s.codex, root.join(".codex").join("config.toml"), Format::CodexToml, "codex", Scope::Project);
    add(s.umbra, home.join(".umbra").join("mcp.json"), Format::McpServers, "umbra", Scope::User);
    add(s.claude, home.join(".claude.json"), Format::McpServers, "claude", Scope::User);
    add(s.claude_desktop, claude_desktop_config(), Format::McpServers, "claude_desktop", Scope::User);
    add(s.codex, codex_home(home).join("config.toml"), Format::CodexToml, "codex", Scope::User);
    add(s.gemini, home.join(".gemini").join("settings.json"), Format::McpServers, "gemini", Scope::User);
    add(s.cursor, home.join(".cursor").join("mcp.json"), Format::McpServers, "cursor", Scope::User);
    add(s.opencode, home.join(".config").join("opencode").join("opencode.json"), Format::Opencode, "opencode", Scope::User);
    list
}

/// Returns Codex's home folder: `CODEX_HOME`, or `~/.codex`.
/// @param home - the user's home folder
pub fn codex_home(home: &Path) -> PathBuf {
    std::env::var("CODEX_HOME").ok().filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| home.join(".codex"))
}

/// Returns Claude Desktop's config file for this platform.
fn claude_desktop_config() -> PathBuf {
    if cfg!(target_os = "macos") {
        return crate::utils::paths::home_dir().join("Library").join("Application Support").join("Claude").join("claude_desktop_config.json");
    }
    app_config_dir().join("Claude").join("claude_desktop_config.json")
}

/// Reads one config file. A missing or unreadable file gives no servers.
/// @param candidate - the file and its format
/// @param ctx - the folders, for variable expansion and the project lookup
fn read_candidate(candidate: &Candidate, ctx: &DiscoveryContext) -> Vec<McpServerDefinition> {
    let Ok(text) = std::fs::read_to_string(&candidate.path) else { return Vec::new() };
    let Some(root) = parse_config_text(&text, candidate.format) else { return Vec::new() };
    let entries = server_entries(&root, candidate.format, ctx);
    entries.iter().filter_map(|(name, entry)| parse_entry(name, entry, &candidate.path, candidate.source, candidate.scope, ctx)).collect()
}

/// Parses JSON, JSON with comments, or TOML into a JSON value.
/// @param text - the file text
/// @param format - the file's format
fn parse_config_text(text: &str, format: Format) -> Option<Value> {
    if let Format::CodexToml = format {
        return toml::from_str::<toml::Value>(text).ok().and_then(|v| serde_json::to_value(v).ok());
    }
    serde_json::from_str(text).ok().or_else(|| serde_json::from_str(&strip_json_comments(text)).ok())
}

/// Picks the server table out of a parsed config file.
/// @param root - the parsed file
/// @param format - the file's format
/// @param ctx - the folders, for the Claude Code project entry
fn server_entries(root: &Value, format: Format, ctx: &DiscoveryContext) -> Map<String, Value> {
    let table = match format {
        Format::McpServers => root.get("mcpServers").cloned(),
        Format::Servers => root.get("servers").cloned(),
        Format::CodexToml => root.get("mcp_servers").cloned(),
        Format::ClaudeJson => claude_project_servers(root, ctx),
        Format::Opencode => opencode_servers(root),
    };
    match table {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

/// Finds this project's entry in `~/.claude.json`. Keys are absolute paths with forward
/// slashes, and the lookup is case sensitive first, then case insensitive, because the file can
/// hold two keys that differ only in letter case.
/// @param root - the parsed `~/.claude.json`
/// @param ctx - the folders
fn claude_project_servers(root: &Value, ctx: &DiscoveryContext) -> Option<Value> {
    let projects = root.get("projects")?.as_object()?;
    let wanted: Vec<String> = [&ctx.cwd, &ctx.project_root].iter().map(|p| p.to_string_lossy().replace('\\', "/")).collect();
    for key in &wanted {
        if let Some(entry) = projects.get(key) {
            return entry.get("mcpServers").cloned();
        }
    }
    if let Some((_, entry)) = projects.iter().find(|(k, _)| wanted.iter().any(|w| w.eq_ignore_ascii_case(k))) {
        return entry.get("mcpServers").cloned();
    }
    // The same folder written another way, such as a Windows short name (`RUNNER~1`) or a link.
    let real: Vec<PathBuf> = [&ctx.cwd, &ctx.project_root].iter().filter_map(|p| p.canonicalize().ok()).collect();
    projects.iter().find(|(k, _)| Path::new(k.as_str()).canonicalize().is_ok_and(|key| real.contains(&key))).and_then(|(_, v)| v.get("mcpServers").cloned())
}

/// Reads opencode's `mcp` key in either layout: servers under `mcp.servers`, or directly under
/// `mcp` as entries that have a `type`.
/// @param root - the parsed opencode.json
fn opencode_servers(root: &Value) -> Option<Value> {
    let mcp = root.get("mcp")?;
    if let Some(Value::Object(servers)) = mcp.get("servers") {
        return Some(Value::Object(servers.clone()));
    }
    let map: Map<String, Value> = mcp.as_object()?.iter().filter(|(_, v)| v.get("type").is_some()).map(|(k, v)| (k.clone(), v.clone())).collect();
    Some(Value::Object(map))
}

/// Turns one server entry, in any harness's shape, into a definition. Returns None for an entry
/// that is turned off or has neither a command nor a URL.
/// @param name - the server name
/// @param entry - the entry object
/// @param origin - the file it came from
/// @param source - the harness that owns the file
/// @param scope - user or project
/// @param ctx - the folders, for variable expansion
pub fn parse_entry(name: &str, entry: &Value, origin: &Path, source: &str, scope: Scope, ctx: &DiscoveryContext) -> Option<McpServerDefinition> {
    if entry.get("enabled") == Some(&Value::Bool(false)) || entry.get("disabled") == Some(&Value::Bool(true)) {
        return None;
    }
    let expand = |s: &str| expand_vars(s, ctx);
    let kind = entry.get("type").and_then(Value::as_str).unwrap_or("").to_lowercase();
    let transport = if let Some(url) = entry.get("httpUrl").and_then(Value::as_str) {
        McpTransport::Http { url: expand(url), headers: headers_of(entry, ctx) }
    } else if let Some(url) = entry.get("url").and_then(Value::as_str) {
        let headers = headers_of(entry, ctx);
        let is_sse = kind == "sse" || (kind.is_empty() && source == "gemini");
        if is_sse { McpTransport::Sse { url: expand(url), headers } } else { McpTransport::Http { url: expand(url), headers } }
    } else {
        stdio_transport(entry, ctx)?
    };
    Some(McpServerDefinition { name: name.to_string(), transport, origin: origin.to_path_buf(), source: source.to_string(), scope })
}

/// Reads a stdio entry. `command` is a string (with `args`) almost everywhere, and an array
/// holding the program and its arguments in opencode.
/// @param entry - the entry object
/// @param ctx - the folders, for variable expansion
fn stdio_transport(entry: &Value, ctx: &DiscoveryContext) -> Option<McpTransport> {
    let expand = |s: &str| expand_vars(s, ctx);
    let (command, mut args) = match entry.get("command")? {
        Value::String(c) => (expand(c), Vec::new()),
        Value::Array(parts) => {
            let parts: Vec<String> = parts.iter().filter_map(Value::as_str).map(&expand).collect();
            (parts.first()?.clone(), parts[1..].to_vec())
        }
        _ => return None,
    };
    if let Some(Value::Array(extra)) = entry.get("args") {
        args.extend(extra.iter().filter_map(Value::as_str).map(&expand));
    }
    let cwd = entry.get("cwd").and_then(Value::as_str).map(expand);
    Some(McpTransport::Stdio { command, args, env: env_of(entry, ctx), cwd })
}

/// Collects a server's environment: `env` (Claude, Cursor, Gemini, Codex, VS Code),
/// `environment` (opencode), and `env_vars` (Codex, names copied from Umbra's own environment).
/// @param entry - the entry object
/// @param ctx - the folders, for variable expansion
fn env_of(entry: &Value, ctx: &DiscoveryContext) -> Vec<(String, String)> {
    let mut env = Vec::new();
    for key in ["env", "environment"] {
        if let Some(Value::Object(map)) = entry.get(key) {
            env.extend(map.iter().map(|(k, v)| (k.clone(), expand_vars(v.as_str().unwrap_or(&v.to_string()), ctx))));
        }
    }
    if let Some(Value::Array(names)) = entry.get("env_vars") {
        for name in names.iter().filter_map(Value::as_str) {
            if let Ok(value) = std::env::var(name) {
                env.push((name.to_string(), value));
            }
        }
    }
    env
}

/// Collects HTTP headers: `headers`, Codex's `http_headers`, `env_http_headers` and
/// `bearer_token_env_var`.
/// @param entry - the entry object
/// @param ctx - the folders, for variable expansion
fn headers_of(entry: &Value, ctx: &DiscoveryContext) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    for key in ["headers", "http_headers"] {
        if let Some(Value::Object(map)) = entry.get(key) {
            headers.extend(map.iter().map(|(k, v)| (k.clone(), expand_vars(v.as_str().unwrap_or(""), ctx))));
        }
    }
    if let Some(Value::Object(map)) = entry.get("env_http_headers") {
        for (header, var) in map {
            if let Some(value) = var.as_str().and_then(|v| std::env::var(v).ok()) {
                headers.push((header.clone(), value));
            }
        }
    }
    if let Some(token) = entry.get("bearer_token_env_var").and_then(Value::as_str).and_then(|v| std::env::var(v).ok()) {
        headers.push(("Authorization".into(), format!("Bearer {token}")));
    }
    headers
}

/// Expands every variable syntax the harnesses use: `${VAR}`, `${VAR:-default}`, `${env:VAR}`,
/// opencode's `{env:VAR}`, and the folder variables `${workspaceFolder}`, `${userHome}` and
/// `${CLAUDE_PROJECT_DIR}`.
/// @param raw - a string from a config file
/// @param ctx - the folders
pub fn expand_vars(raw: &str, ctx: &DiscoveryContext) -> String {
    let root = ctx.project_root.to_string_lossy().to_string();
    let home = ctx.home.to_string_lossy().to_string();
    let replaced = raw.replace("${workspaceFolder}", &root).replace("${CLAUDE_PROJECT_DIR}", &root).replace("${userHome}", &home);
    let opencode = regex::Regex::new(r"(^|[^$])\{env:([A-Za-z_][A-Za-z0-9_]*)\}").expect("valid regex");
    let replaced = opencode.replace_all(&replaced, |c: &regex::Captures| format!("{}{}", &c[1], std::env::var(&c[2]).unwrap_or_default()));
    expand_env(&replaced)
}

/// Removes `//` and `/* */` comments outside strings, for opencode.jsonc.
/// @param text - JSON with comments
fn strip_json_comments(text: &str) -> String {
    let (mut out, mut chars) = (String::new(), text.chars().peekable());
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(n) = chars.next() { out.push(n); }
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '/' && chars.peek() == Some(&'/') {
            while chars.peek().is_some_and(|&n| n != '\n') { chars.next(); }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(n) = chars.next() { if n == '*' && chars.peek() == Some(&'/') { chars.next(); break; } }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::config::SourcesConfig;
    use serde_json::json;

    fn ctx() -> DiscoveryContext {
        DiscoveryContext { home: PathBuf::from("/home/u"), cwd: PathBuf::from("/p"), project_root: PathBuf::from("/p"), sources: SourcesConfig::default() }
    }

    #[test]
    fn reads_every_entry_shape() {
        let c = ctx();
        let origin = Path::new("x");
        let stdio = parse_entry("a", &json!({"command": "node", "args": ["s.js"], "env": {"K": "v"}}), origin, "claude", Scope::User, &c).unwrap();
        assert!(matches!(stdio.transport, McpTransport::Stdio { ref command, .. } if command == "node"));
        let opencode = parse_entry("b", &json!({"type": "local", "command": ["npx", "-y", "pkg"]}), origin, "opencode", Scope::User, &c).unwrap();
        assert!(matches!(opencode.transport, McpTransport::Stdio { ref args, .. } if args == &vec!["-y".to_string(), "pkg".to_string()]));
        let http = parse_entry("c", &json!({"type": "http", "url": "https://x/mcp"}), origin, "claude", Scope::User, &c).unwrap();
        assert!(matches!(http.transport, McpTransport::Http { .. }));
        let gemini_sse = parse_entry("d", &json!({"url": "http://x/sse"}), origin, "gemini", Scope::User, &c).unwrap();
        assert!(matches!(gemini_sse.transport, McpTransport::Sse { .. }));
        assert!(parse_entry("e", &json!({"command": "x", "enabled": false}), origin, "opencode", Scope::User, &c).is_none());
    }

    #[test]
    fn strips_comments_but_not_urls() {
        let text = "{ // note\n \"url\": \"http://x\" /* c */ }";
        let parsed: Value = serde_json::from_str(&strip_json_comments(text)).unwrap();
        assert_eq!(parsed["url"], "http://x");
    }
}

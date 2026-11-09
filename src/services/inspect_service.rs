//! The listing subcommands: what Umbra found on disk and where it found it.

use anyhow::Result;
use serde_json::json;

use super::context_service::{discovery_context, load_run_context};
use super::mcp::McpManager;
use super::run_service::prepare_config;
use super::session_service::{describe_session, listed_sessions};
use crate::cli::{InspectArgs, McpArgs, SessionsArgs};
use crate::models::context::McpTransport;
use crate::utils::paths::{absolute_path, umbra_home};

/// Prints the skills, highest precedence first.
/// @param args - the listing options
pub fn list_skills(args: &InspectArgs) -> Result<()> {
    let (config, cwd) = prepare_config(&args.common)?;
    let context = load_run_context(&config, &discovery_context(&config, &cwd));
    if args.json {
        println!("{}", serde_json::to_string_pretty(&context.skills)?);
        return Ok(());
    }
    println!("{} skills\n", context.skills.len());
    for skill in &context.skills {
        println!("{:<28} {:<14} {:?}\n    {}\n    {}", skill.name, skill.source, skill.scope, first_line(&skill.description, 110), skill.path.display());
    }
    Ok(())
}

/// Prints the instruction files in the order they go into the prompt.
/// @param args - the listing options
pub fn list_instructions(args: &InspectArgs) -> Result<()> {
    let (config, cwd) = prepare_config(&args.common)?;
    let context = load_run_context(&config, &discovery_context(&config, &cwd));
    if args.json {
        println!("{}", serde_json::to_string_pretty(&context.instructions)?);
        return Ok(());
    }
    let total: usize = context.instructions.iter().map(|i| i.content.len()).sum();
    println!("{} instruction files, {} bytes\n", context.instructions.len(), total);
    for file in &context.instructions {
        println!("{:<10} {:<8?} {:>8} bytes  {}", file.source, file.scope, file.content.len(), file.path.display());
    }
    Ok(())
}

/// Prints the slash commands.
/// @param args - the listing options
pub fn list_commands(args: &InspectArgs) -> Result<()> {
    let (config, cwd) = prepare_config(&args.common)?;
    let context = load_run_context(&config, &discovery_context(&config, &cwd));
    if args.json {
        println!("{}", serde_json::to_string_pretty(&context.commands)?);
        return Ok(());
    }
    println!("{} commands\n", context.commands.len());
    for command in &context.commands {
        println!("/{:<27} {:<10} {}", command.name, command.source, command.path.display());
    }
    Ok(())
}

/// Prints the MCP servers and, with `--connect`, starts them and prints their tools.
/// @param args - the MCP listing options
pub fn list_mcp(args: &McpArgs) -> Result<()> {
    let (config, cwd) = prepare_config(&args.inspect.common)?;
    let context = load_run_context(&config, &discovery_context(&config, &cwd));
    if !args.connect {
        return print_mcp_definitions(&context.mcp_servers, args.inspect.json);
    }
    let mut manager = McpManager::connect_all(&context.mcp_servers, &config.mcp, &umbra_home().join("logs").join("mcp"));
    let tools = manager.tools();
    if args.inspect.json {
        let failures: Vec<_> = manager.failures.iter().map(|f| json!({ "name": f.name, "error": f.error })).collect();
        println!("{}", serde_json::to_string_pretty(&json!({ "connected": manager.connected(), "failures": failures, "tools": tools }))?);
    } else {
        for (name, version, count) in manager.connected() {
            println!("{name:<20} connected, protocol {version}, {count} tools");
        }
        for failure in &manager.failures {
            println!("{:<20} FAILED: {}", failure.name, first_line(&failure.error, 160));
        }
        println!();
        for tool in &tools {
            println!("{:<50} {}", tool.qualified_name(), first_line(&tool.description, 90));
        }
    }
    manager.close();
    Ok(())
}

/// Prints the MCP definitions without starting them. Environment values are never printed.
/// @param servers - the definitions
/// @param as_json - print JSON
fn print_mcp_definitions(servers: &[crate::models::context::McpServerDefinition], as_json: bool) -> Result<()> {
    if as_json {
        let list: Vec<_> = servers.iter().map(|s| json!({ "name": s.name, "transport": describe_transport(&s.transport), "source": s.source, "scope": s.scope, "origin": s.origin })).collect();
        println!("{}", serde_json::to_string_pretty(&list)?);
        return Ok(());
    }
    println!("{} MCP servers (first one found wins for each name)\n", servers.len());
    for server in servers {
        println!("{:<20} {:<15} {:<8?} {}\n    {}", server.name, server.source, server.scope, server.origin.display(), describe_transport(&server.transport));
    }
    Ok(())
}

/// Describes a transport in one line, leaving out environment values and headers.
/// @param transport - the transport
fn describe_transport(transport: &McpTransport) -> String {
    match transport {
        McpTransport::Stdio { command, args, env, .. } => format!("stdio: {command} {} ({} env vars)", args.join(" "), env.len()),
        McpTransport::Http { url, headers } => format!("http: {url} ({} headers)", headers.len()),
        McpTransport::Sse { url, headers } => format!("sse: {url} ({} headers)", headers.len()),
    }
}

/// Prints the merged config as TOML.
/// @param args - the listing options
pub fn show_config(args: &InspectArgs) -> Result<()> {
    let (mut config, _) = prepare_config(&args.common)?;
    for provider in config.providers.values_mut() {
        if !provider.api_key.is_empty() {
            provider.api_key = "(set)".into();
        }
    }
    if args.json {
        println!("{}", serde_json::to_string_pretty(&config)?);
    } else {
        let value: toml::Value = serde_json::from_value(strip_nulls(serde_json::to_value(&config)?))?;
        println!("{}", toml::to_string_pretty(&value)?);
    }
    Ok(())
}

/// Removes null values, which TOML cannot hold.
/// @param value - a JSON value
fn strip_nulls(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(map.into_iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| (k, strip_nulls(v))).collect()),
        other => other,
    }
}

/// Returns the first line of text, shortened.
/// @param text - the text
/// @param limit - the most characters to keep
fn first_line(text: &str, limit: usize) -> String {
    let line = text.lines().next().unwrap_or("");
    if line.chars().count() > limit { format!("{}...", line.chars().take(limit).collect::<String>()) } else { line.to_string() }
}

/// Prints saved sessions, newest first: this folder's, or every folder's with `--all`.
/// @param args - the listing options
pub fn list_sessions(args: &SessionsArgs) -> Result<()> {
    let cwd = match &args.cwd {
        Some(dir) => absolute_path(dir),
        None => std::env::current_dir()?,
    };
    let sessions = listed_sessions(&cwd, args.all);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&sessions)?);
        return Ok(());
    }
    let place = if args.all { "in every folder".to_string() } else { format!("in {}", cwd.display()) };
    println!("{} sessions {place}. Resume one with umbra --resume <id>\n", sessions.len());
    for session in &sessions {
        println!("{}", describe_session(session));
        if args.all {
            println!("    {}", session.cwd);
        }
    }
    Ok(())
}

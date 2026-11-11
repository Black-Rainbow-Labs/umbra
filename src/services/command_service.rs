//! Slash commands: the built in commands of the terminal, the custom commands and skills found on
//! disk, the list the command popup completes from, and what a typed line means. One function
//! decides that for the first prompt, for follow up prompts and for lines typed while the agent
//! works, so a custom command works on every prompt.

use super::context_service::{expand_slash_command, RunContext};
use crate::ui::live::CommandEntry;

/// One built in command.
pub struct Builtin {
    pub name: &'static str,
    /// The arguments it takes, for the help text.
    pub args: &'static str,
    pub description: &'static str,
}

/// The built in commands, in the order the help lists them.
pub const BUILTINS: &[Builtin] = &[
    Builtin { name: "config", args: "", description: "Open the configuration" },
    Builtin { name: "model", args: "[name]", description: "Pick the model for this session, or switch to the named one" },
    Builtin { name: "mcp", args: "", description: "List the MCP servers and turn them on or off for this session" },
    Builtin { name: "mode", args: "[confirm, yolo, human]", description: "Switch the mode, or pick one" },
    Builtin { name: "clear", args: "", description: "Start a new session with an empty history; the old one stays in the resume list" },
    Builtin { name: "compact", args: "[focus]", description: "Summarize the history now" },
    Builtin { name: "resume", args: "", description: "Pick a saved session to continue" },
    Builtin { name: "skills", args: "", description: "Pick a skill to run" },
    Builtin { name: "help", args: "", description: "List the commands and keys" },
    Builtin { name: "quit", args: "", description: "End the session" },
];

/// A built in command, read from a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinCommand {
    Config,
    Model(Option<String>),
    Mcp,
    Mode(Option<String>),
    Clear,
    Compact(String),
    Resume(Option<String>),
    Skills,
    Help,
    Quit,
}

/// What a typed line means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineAction {
    Builtin(BuiltinCommand),
    /// Send this to the model: the line itself, or the custom command or skill it named.
    SendToModel(String),
}

/// Reads a built in command from a line, or None when the line is not one.
/// @param line - what the person typed
pub fn parse_builtin(line: &str) -> Option<BuiltinCommand> {
    let rest = line.trim().strip_prefix('/')?;
    let (name, argument) = rest.split_once(char::is_whitespace).map(|(n, a)| (n, a.trim())).unwrap_or((rest, ""));
    let argument = (!argument.is_empty()).then(|| argument.to_string());
    Some(match name {
        "config" => BuiltinCommand::Config,
        "model" => BuiltinCommand::Model(argument),
        "mcp" => BuiltinCommand::Mcp,
        "mode" => BuiltinCommand::Mode(argument),
        "clear" => BuiltinCommand::Clear,
        "compact" => BuiltinCommand::Compact(argument.unwrap_or_default()),
        "resume" => BuiltinCommand::Resume(argument),
        "skills" => BuiltinCommand::Skills,
        "help" => BuiltinCommand::Help,
        "quit" | "exit" => BuiltinCommand::Quit,
        _ => return None,
    })
}

/// What a line means: a built in command, a custom command or skill expanded into its prompt, or
/// the line itself.
/// @param line - what the person typed
/// @param context - the loaded commands and skills
pub fn handle_line(line: &str, context: &RunContext) -> LineAction {
    match parse_builtin(line) {
        Some(command) => LineAction::Builtin(command),
        None => LineAction::SendToModel(expand_slash_command(line, context)),
    }
}

/// The custom commands and skills, for the popup and for an editor's command list. A skill with
/// the same name as a custom command is left out.
/// @param context - the loaded commands and skills
pub fn custom_entries(context: &RunContext) -> Vec<CommandEntry> {
    let mut entries: Vec<CommandEntry> = context.commands.iter().map(|c| CommandEntry { name: c.name.clone(), kind: "custom", description: first_line(&c.description, &format!("Run {}", c.path.display())) }).collect();
    for skill in &context.skills {
        if !entries.iter().any(|e| e.name == skill.name) {
            entries.push(CommandEntry { name: skill.name.clone(), kind: "skill", description: first_line(&skill.description, "A skill") });
        }
    }
    entries
}

/// Every command the popup offers: the built in commands, then the custom commands and skills.
/// A built in name wins over a custom command or skill of the same name.
/// @param context - the loaded commands and skills
pub fn command_entries(context: &RunContext) -> Vec<CommandEntry> {
    let mut entries: Vec<CommandEntry> = BUILTINS.iter().map(|b| CommandEntry { name: b.name.into(), kind: "command", description: b.description.into() }).collect();
    for entry in custom_entries(context) {
        if !entries.iter().any(|e| e.name == entry.name) {
            entries.push(entry);
        }
    }
    entries
}

/// The commands that match typed text: names that start with it first, then names that contain
/// it, ignoring case.
/// @param typed - the text after the `/`
/// @param entries - every command
pub fn complete(typed: &str, entries: &[CommandEntry]) -> Vec<CommandEntry> {
    let needle = typed.to_lowercase();
    let mut found: Vec<CommandEntry> = entries.iter().filter(|e| e.name.to_lowercase().starts_with(&needle)).cloned().collect();
    found.extend(entries.iter().filter(|e| !e.name.to_lowercase().starts_with(&needle) && e.name.to_lowercase().contains(&needle)).cloned());
    found
}

/// The help text: the commands and the keys.
pub fn help_text() -> String {
    let mut lines = vec!["Commands".to_string()];
    for builtin in BUILTINS {
        let usage = if builtin.args.is_empty() { format!("/{}", builtin.name) } else { format!("/{} {}", builtin.name, builtin.args) };
        lines.push(format!("  {usage:<34} {}", builtin.description));
    }
    lines.push("  /<command> or /<skill>             run a custom command or a skill".into());
    lines.push(String::new());
    lines.push("Keys".into());
    for (keys, what) in [("Enter", "send the line"), ("Tab, Shift+Tab", "move focus through the transcript; Enter or Space opens"), ("Up, Down", "earlier prompts"), ("PageUp, PageDown, wheel", "scroll"), ("Ctrl+Home, Ctrl+End", "the top and the bottom"), ("Ctrl+A, Ctrl+E, Ctrl+K, Ctrl+W, Ctrl+U", "edit the line"), ("Esc", "close a panel, give focus back to the input"), ("Ctrl+C", "interrupt the agent"), ("drag", "select text and copy it")] {
        lines.push(format!("  {keys:<34} {what}"));
    }
    lines.join("\n")
}

/// The first line of a description, at most 200 characters, or the fallback when there is none.
/// @param text - the description
/// @param fallback - used when it is empty
fn first_line(text: &str, fallback: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty() { fallback.to_string() } else { line.chars().take(200).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_built_in_commands_and_their_arguments() {
        assert_eq!(parse_builtin("/model opus"), Some(BuiltinCommand::Model(Some("opus".into()))));
        assert_eq!(parse_builtin("  /compact keep tests "), Some(BuiltinCommand::Compact("keep tests".into())));
        assert_eq!(parse_builtin("/mode"), Some(BuiltinCommand::Mode(None)));
        assert_eq!(parse_builtin("/my-command"), None);
        assert_eq!(parse_builtin("fix the build"), None);
    }

    #[test]
    fn completes_by_prefix_then_by_substring() {
        let entries = command_entries(&RunContext::default());
        let names: Vec<String> = complete("co", &entries).into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["config", "compact"]);
        let names: Vec<String> = complete("m", &entries).into_iter().map(|e| e.name).collect();
        assert_eq!(&names[..3], &["model", "mcp", "mode"]);
    }
}

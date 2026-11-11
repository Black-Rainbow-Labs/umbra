//! The built in commands the view runs itself when Enter is pressed: the ones that only open
//! something on screen run at once, even while the agent works. The ones that need the agent to be
//! idle are refused with a notice while it works, and otherwise sent on as a line.

use super::block::Block;
use super::overlay::config_modal::ConfigModal;
use super::overlay::mcp_panel::McpPanel;
use super::overlay::pickers;
use super::overlay::Effect;
use super::view::{KeyOutcome, View};
use crate::services::command_service::{help_text, parse_builtin, BuiltinCommand};
use crate::services::interaction_service::Tone;
use crate::services::work_clock;

/// Runs a built in command typed on the input line when the view handles it, and returns None for
/// any other line, which is sent on.
/// @param view - the view
/// @param line - what the person typed
pub fn view_command(view: &mut View, line: &str) -> Option<(KeyOutcome, Vec<Effect>)> {
    let command = parse_builtin(line)?;
    let working = work_clock::is_running();
    match command {
        BuiltinCommand::Config => view.overlay = Some(ConfigModal::open()),
        BuiltinCommand::Model(None) => view.overlay = Some(pickers::model_picker()),
        BuiltinCommand::Model(Some(name)) => {
            if let Err(error) = pickers::switch_model(&name) {
                notice(view, Tone::Failure, &error);
            } else {
                notice(view, Tone::Info, &format!("model {name} from the next model call"));
            }
        }
        BuiltinCommand::Mcp => view.overlay = Some(Box::new(McpPanel::new())),
        BuiltinCommand::Mode(None) => view.overlay = Some(pickers::mode_picker()),
        BuiltinCommand::Mode(Some(word)) => match pickers::parse_mode(&word) {
            Some(mode) => {
                pickers::switch_mode(mode);
                notice(view, Tone::Info, &format!("{} mode", pickers::mode_name(mode)));
            }
            None => notice(view, Tone::Warning, &format!("unknown mode '{word}'. Use confirm, yolo or human")),
        },
        BuiltinCommand::Skills => view.overlay = Some(pickers::skills_picker()),
        BuiltinCommand::Help => {
            view.push_block(Block::Text(String::new()));
            view.push_block(Block::Notice { tone: Tone::Info, marker: "?".into(), text: help_text() });
        }
        BuiltinCommand::Resume(None) if !working => view.overlay = Some(pickers::session_picker(resumable_sessions())),
        BuiltinCommand::Clear | BuiltinCommand::Resume(_) if working => notice(view, Tone::Warning, "wait for the agent to finish, or press Ctrl+C, before starting another session"),
        BuiltinCommand::Quit if working => {
            super::live::request(super::live::UiRequest::Quit);
            notice(view, Tone::Info, "the session ends after the current step");
        }
        BuiltinCommand::Clear | BuiltinCommand::Resume(_) | BuiltinCommand::Compact(_) | BuiltinCommand::Quit | BuiltinCommand::Update => return None,
    }
    Some((KeyOutcome::None, Vec::new()))
}

/// The sessions the picker offers: this folder's, newest first, without the one on screen.
fn resumable_sessions() -> Vec<crate::models::session::SessionIndexEntry> {
    let folder = super::live::header().map(|h| h.cwd).unwrap_or_default();
    let current = super::live::session();
    crate::services::session_service::listed_sessions(&folder, false).into_iter().filter(|s| s.id != current).take(50).collect()
}

/// Adds a notice to the transcript.
/// @param view - the view
/// @param tone - how much attention it asks for
/// @param text - the notice
fn notice(view: &mut View, tone: Tone, text: &str) {
    view.push_block(Block::Notice { tone, marker: "·".into(), text: text.to_string() });
}

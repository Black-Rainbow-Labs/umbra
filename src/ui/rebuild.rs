//! Builds the transcript of a resumed session from its saved messages: the person's prompts,
//! the replies, and the commands as collapsed command groups, the same blocks the live session
//! drew.

use std::time::Duration;

use super::block::{Block, CommandRun, RunState};
use super::transcript::Transcript;
use crate::models::action::{Action, Observation};
use crate::models::message::{Message, Role};
use crate::services::terminal_events::{finish_run, run_kind};

/// The prefix of a line the person typed while the agent worked.
const QUEUED_PREFIX: &str = "The user sent this message while you were working: ";

/// Adds the blocks of saved messages to a transcript.
/// @param transcript - the transcript
/// @param messages - the session's history
pub fn rebuild(transcript: &mut Transcript, messages: &[Message]) {
    let mut pending: Vec<(u64, usize)> = Vec::new();
    for message in messages {
        match message.role {
            Role::System | Role::Exit => {}
            Role::User | Role::Tool if is_observation(message) => finish_next(transcript, &mut pending, message),
            Role::User => add_user(transcript, message),
            Role::Assistant => pending = add_reply(transcript, message),
            _ => {}
        }
    }
    transcript.close_group();
}

/// True for a message that carries an action's result.
/// @param message - a saved message
fn is_observation(message: &Message) -> bool {
    message.role == Role::Tool || message.extra.get("raw_output").is_some()
}

/// Adds a prompt the person typed. Format errors and other notes to the model are left out.
/// @param transcript - the transcript
/// @param message - a user message
fn add_user(transcript: &mut Transcript, message: &Message) {
    if let Some(text) = message.extra["user_text"].as_str() {
        transcript.push(Block::UserPrompt { marker: "❯".into(), text: text.to_string() });
        return;
    }
    if message.extra["interrupt_type"] == "UserMessage" {
        let text = message.content.strip_prefix(QUEUED_PREFIX).unwrap_or(&message.content);
        transcript.push(Block::UserPrompt { marker: "›".into(), text: text.to_string() });
    }
}

/// Adds a reply's text and its actions as runs, and returns where each run went, in order.
/// @param transcript - the transcript
/// @param message - an assistant message
fn add_reply(transcript: &mut Transcript, message: &Message) -> Vec<(u64, usize)> {
    if !message.content.trim().is_empty() {
        transcript.push(Block::Reply { markdown: message.content.trim().to_string() });
    }
    let actions: Vec<Action> = serde_json::from_value(message.extra["actions"].clone()).unwrap_or_default();
    actions.iter().map(|action| {
        let mut run = CommandRun::running(run_kind(action));
        run.state = RunState::Pending;
        transcript.add_run(run)
    }).collect()
}

/// Fills the next waiting run in from a saved result.
/// @param transcript - the transcript
/// @param pending - the runs still waiting for a result, in order
/// @param message - the result message
fn finish_next(transcript: &mut Transcript, pending: &mut Vec<(u64, usize)>, message: &Message) {
    if pending.is_empty() {
        return;
    }
    let (group, index) = pending.remove(0);
    let extra = &message.extra;
    let observation = Observation { output: extra["raw_output"].as_str().unwrap_or("").to_string(), returncode: extra["returncode"].as_i64().unwrap_or(0) as i32, exception_info: extra["exception_info"].as_str().unwrap_or("").to_string(), duration_ms: extra["duration_ms"].as_u64(), ..Default::default() };
    if let Some(run) = transcript.group_mut(group).and_then(|g| g.runs.get_mut(index)) {
        finish_run(run, &observation);
        if observation.duration_ms.is_none() {
            run.duration = Some(Duration::ZERO);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_saved_history_becomes_prompts_replies_and_collapsed_groups() {
        let assistant = Message::new(Role::Assistant, "Looking.").with_extra(json!({ "actions": [{ "kind": "bash", "command": "ls" }, { "kind": "bash", "command": "pwd" }] }));
        let messages = vec![
            Message::system("s"),
            Message::user("instance").with_extra(json!({ "user_text": "list the files" })),
            assistant,
            Message::tool("a", "x").with_extra(json!({ "raw_output": "a.txt", "returncode": 0, "duration_ms": 1200 })),
            Message::tool("b", "x").with_extra(json!({ "raw_output": "/w", "returncode": 1 })),
            Message::new(Role::Assistant, "Two files."),
        ];
        let mut transcript = Transcript::default();
        rebuild(&mut transcript, &messages);
        let kinds: Vec<&str> = transcript.entries().map(|e| match &e.block { Block::UserPrompt { .. } => "prompt", Block::Reply { .. } => "reply", Block::CommandGroup(_) => "group", _ => "other" }).collect();
        assert_eq!(kinds, vec!["prompt", "reply", "group", "reply"]);
        let group = transcript.last_group().unwrap();
        let Some(Block::CommandGroup(g)) = transcript.block(group) else { panic!() };
        assert!(!g.expanded);
        assert_eq!(g.runs[0].duration, Some(Duration::from_millis(1200)));
        assert!(g.runs[1].failed());
    }
}

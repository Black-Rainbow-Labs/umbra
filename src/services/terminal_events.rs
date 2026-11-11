//! What the agent tells the fullscreen view while it works. Each action becomes a run in the open
//! command group of the transcript: it starts as running with a live time, and is filled in with
//! its output, exit code and duration when it finishes. The inline view prints the run itself, so
//! there this does nothing.

use std::collections::HashMap;
use std::time::Duration;

use super::agent_events::AgentEvents;
use crate::models::action::{Action, Observation};
use crate::ui::block::{CommandRun, RunKind, RunState};
use crate::utils::screen;

/// The longest MCP argument text a run row keeps.
const ARGS_CHARS: usize = 400;

/// The fullscreen view's event hook.
#[derive(Default)]
pub struct TerminalEvents {
    /// The group and run each action of the current reply became, by action position.
    runs: HashMap<usize, (u64, usize)>,
}

/// What kind of run an action is.
/// @param action - the action
pub fn run_kind(action: &Action) -> RunKind {
    match action {
        Action::Bash { command, .. } => RunKind::Shell { command: command.clone() },
        Action::Mcp { server, tool, arguments, .. } => RunKind::Mcp { server: server.clone(), tool: tool.clone(), args: arguments.to_string().chars().take(ARGS_CHARS).collect() },
    }
}

/// Fills a run in from its observation.
/// @param run - the run
/// @param observation - what the action returned
pub fn finish_run(run: &mut CommandRun, observation: &Observation) {
    let measured = match run.state {
        RunState::Running { since } => Some(since.elapsed()),
        _ => None,
    };
    run.state = RunState::Done;
    run.output = observation.output.clone();
    run.exit_code = Some(observation.returncode);
    run.error = (!observation.exception_info.is_empty()).then(|| observation.exception_info.clone());
    run.duration = observation.duration_ms.map(Duration::from_millis).or(measured).or(run.duration);
}

impl AgentEvents for TerminalEvents {
    /// A new reply: its actions start fresh positions.
    /// @param _content - the reply text, shown by the reply block
    /// @param _actions - the actions it asked for
    fn reply(&mut self, _content: &str, _actions: &[Action]) {
        self.runs.clear();
    }

    /// Adds the action to the open command group as running.
    /// @param index - its position in the reply's actions
    /// @param action - the action
    fn action_started(&mut self, index: usize, action: &Action) {
        if !screen::is_active() {
            return;
        }
        if let Some(ids) = screen::with_transcript(|t| t.add_run(CommandRun::running(run_kind(action)))) {
            self.runs.insert(index, ids);
        }
    }

    /// Fills the run in with what the action returned.
    /// @param index - its position in the reply's actions
    /// @param _action - the action
    /// @param observation - what it returned
    fn action_finished(&mut self, index: usize, _action: &Action, observation: &Observation) {
        let Some(&(group, position)) = self.runs.get(&index) else { return };
        screen::with_transcript(|t| {
            if let Some(run) = t.group_mut(group).and_then(|g| g.runs.get_mut(position)) {
                finish_run(run, observation);
            }
        });
    }

    /// Marks the run as not run, adding it to the group when it never started.
    /// @param index - its position in the reply's actions
    /// @param action - the action
    /// @param reason - what the model is told
    fn action_not_run(&mut self, index: usize, action: &Action, reason: &str) {
        if !screen::is_active() {
            return;
        }
        let state = RunState::NotRun { reason: reason.to_string() };
        match self.runs.get(&index).copied() {
            Some((group, position)) => {
                screen::with_transcript(|t| {
                    if let Some(run) = t.group_mut(group).and_then(|g| g.runs.get_mut(position)) {
                        run.state = state;
                    }
                });
            }
            None => {
                let mut run = CommandRun::running(run_kind(action));
                run.state = state;
                screen::with_transcript(|t| t.add_run(run));
            }
        }
    }
}

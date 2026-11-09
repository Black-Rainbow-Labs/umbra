//! What the agent loop tells a front end while it works. The terminal prints the run itself and
//! uses the hook that does nothing; the ACP server uses one that sends session updates to the
//! editor and asks the editor before a command runs.

use super::interaction_service::Decision;
use crate::models::action::{Action, Observation};

/// The events of a run, each called on the thread that runs the agent. Every method has a default
/// that does nothing, so a front end only writes the ones it needs.
pub trait AgentEvents {
    /// The model replied. Called once per model call that produced usable actions.
    /// @param _content - the reply text
    /// @param _actions - the actions it asked for, in order
    fn reply(&mut self, _content: &str, _actions: &[Action]) {}

    /// An action is about to run.
    /// @param _index - its position in the reply's actions
    /// @param _action - the action
    fn action_started(&mut self, _index: usize, _action: &Action) {}

    /// An action finished.
    /// @param _index - its position in the reply's actions
    /// @param _action - the action
    /// @param _observation - what it returned
    fn action_finished(&mut self, _index: usize, _action: &Action, _observation: &Observation) {}

    /// An action was not run: it was refused, the turn was cancelled, or an earlier action ended
    /// the run.
    /// @param _index - its position in the reply's actions
    /// @param _action - the action
    /// @param _reason - what the model is told
    fn action_not_run(&mut self, _index: usize, _action: &Action, _reason: &str) {}

    /// A notice the terminal shows under a heading, such as a compaction or a format error.
    /// @param _label - the heading
    /// @param _text - the notice
    fn notice(&mut self, _label: &str, _text: &str) {}

    /// The numbers after a model call.
    /// @param _used - the estimated size of the next request in tokens
    /// @param _window - the model's context window in tokens
    /// @param _cost - the session's cost so far in dollars
    fn usage(&mut self, _used: u64, _window: u64, _cost: f64) {}

    /// Decides whether actions run. None hands the decision to the terminal's confirm prompt.
    /// @param _actions - the actions the model asked for
    fn confirm(&mut self, _actions: &[Action]) -> Option<Decision> {
        None
    }
}

/// The hook the terminal uses: it does nothing, because the terminal prints the run itself.
pub struct NoEvents;

impl AgentEvents for NoEvents {}

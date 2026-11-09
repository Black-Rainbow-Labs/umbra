//! The person at the keyboard: confirming actions, typing commands in human mode, giving a
//! follow up task when the agent finishes, and printing the run as it happens.

use regex::Regex;

use crate::models::action::{Action, Observation};
use crate::models::config::Mode;
use crate::utils::console::{clip, heading, is_quiet, paint, prompt_line, read_line, stdin_is_terminal, Input};

/// What the person decided about a set of actions.
pub enum Decision {
    /// Run them.
    Run,
    /// Do not run them, and tell the model this.
    Reject(String),
    /// Ctrl+C at the prompt: do not run them, and show the comment prompt.
    Interrupted,
}

/// What the person typed at the comment prompt after a Ctrl+C.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Comment {
    /// Send this to the model as `Interrupted by user: <text>` and carry on.
    Text(String),
    /// `/compact [focus]`: compact now, then ask again.
    Compact(String),
    /// End the run.
    Quit,
}

/// What the model is told when the person interrupts and types nothing, as in mini-swe-agent.
pub const TEMPORARY_INTERRUPTION: &str = "Temporary interruption caught.";

/// The comment prompt, mini-swe-agent's wording.
const COMMENT_PROMPT: &str = "\nInterrupted. Type a comment/command (/h for available commands)\n> ";

/// What the person typed in human mode.
pub enum HumanInput {
    Command(String),
    /// Hand control back to the model in the given mode.
    SwitchMode(Mode),
    Quit,
    /// Ctrl+C at the prompt.
    Interrupted,
}

/// The interactive state of a run.
pub struct Interaction {
    pub mode: Mode,
    whitelist: Vec<Regex>,
    pub confirm_exit: bool,
    /// An empty line at the follow up prompt asks again instead of ending the run.
    pub keep_open: bool,
    /// False when stdin is not a terminal; then Umbra never asks and never waits.
    pub interactive: bool,
}

impl Interaction {
    /// Builds the interaction settings.
    /// @param mode - confirm, yolo or human
    /// @param whitelist - regexes for commands that run without asking
    /// @param confirm_exit - ask for a follow up task when the agent finishes
    pub fn new(mode: Mode, whitelist: &[String], confirm_exit: bool) -> Self {
        let whitelist = whitelist.iter().filter_map(|p| Regex::new(&format!("^(?:{p})")).ok()).collect();
        let interactive = stdin_is_terminal();
        Interaction { mode, whitelist, confirm_exit, keep_open: false, interactive }
    }

    /// Asks the person to approve actions in confirm mode. Whitelisted commands, yolo mode and a
    /// run with no terminal never ask.
    /// @param actions - the actions the model wants to run
    pub fn confirm(&mut self, actions: &[Action]) -> Decision {
        if self.mode != Mode::Confirm || !self.interactive || actions.iter().all(|a| self.is_whitelisted(a)) {
            return Decision::Run;
        }
        loop {
            let answer = match prompt_line("Run? [Enter] yes, /y yolo, /u human mode, /h help, or type why not: ") {
                Input::Line(line) => line,
                Input::Eof => String::new(),
                Input::Interrupted => return Decision::Interrupted,
            };
            match answer.trim() {
                "" => return Decision::Run,
                "/y" => {
                    self.mode = Mode::Yolo;
                    return Decision::Run;
                }
                "/u" => {
                    self.mode = Mode::Human;
                    return Decision::Reject("Commands not executed. Switching to human mode".into());
                }
                "/h" => print_help(),
                text => return Decision::Reject(format!("Commands not executed. The user rejected your commands with the following message: {text}")),
            }
        }
    }

    /// Reads a command typed by the person in human mode.
    pub fn read_human_command(&mut self) -> HumanInput {
        loop {
            let line = match prompt_line("command (/c confirm mode, /y yolo mode, /q quit): ") {
                Input::Line(line) => line,
                Input::Eof => return HumanInput::Quit,
                Input::Interrupted => return HumanInput::Interrupted,
            };
            match line.trim() {
                "" => continue,
                "/q" => return HumanInput::Quit,
                "/c" => return HumanInput::SwitchMode(Mode::Confirm),
                "/y" => return HumanInput::SwitchMode(Mode::Yolo),
                "/h" => print_help(),
                command => return HumanInput::Command(command.to_string()),
            }
        }
    }

    /// When the agent finishes, asks for a follow up task. Returns None to end the run. With
    /// keep_open an empty line asks again, so only Ctrl+C or the end of input ends the run.
    pub fn follow_up_task(&self) -> Option<String> {
        if !self.confirm_exit || !self.interactive {
            return None;
        }
        let prompt = if self.keep_open { "Agent wants to finish. Type a new task: " } else { "Agent wants to finish. Type a new task, or press Enter to quit: " };
        loop {
            let line = match prompt_line(prompt) {
                Input::Line(line) => line,
                // Ctrl+C at an idle prompt is a stop with nothing to stop. With keep_open it asks
                // again, so a stop pressed while the agent waits cannot end the session.
                Input::Interrupted if self.keep_open => continue,
                Input::Interrupted | Input::Eof => return None,
            };
            let line = line.trim();
            if !line.is_empty() {
                return Some(line.to_string());
            }
            if !self.keep_open {
                return None;
            }
        }
    }

    /// Asks for higher limits when one is reached. Returns the new step and cost limits.
    /// @param steps - the current step limit
    /// @param cost - the current cost limit
    pub fn raise_limits(&self, steps: u32, cost: f64) -> Option<(u32, f64)> {
        if !self.interactive {
            return None;
        }
        let line = read_line(&format!("Limit reached (steps {steps}, cost ${cost:.2}). New limits as `steps,cost` (0 means none), or Enter to stop: "))?;
        let (s, c) = line.trim().split_once(',')?;
        Some((s.trim().parse().ok()?, c.trim().parse().ok()?))
    }

    /// Asks what to do after a Ctrl+C, the way mini-swe-agent does. A second Ctrl+C, `/q` or the
    /// end of input ends the run. Mode commands switch the mode and send the temporary message.
    pub fn comment_prompt(&mut self) -> Comment {
        let mut prompt = COMMENT_PROMPT.to_string();
        loop {
            let line = match prompt_line(&prompt) {
                Input::Line(line) => line,
                // A second Ctrl+C ends the run, unless keep_open is set: then it carries on, so
                // pressing stop twice in a board session never ends the agent.
                Input::Interrupted if self.keep_open => return Comment::Text(TEMPORARY_INTERRUPTION.into()),
                Input::Eof | Input::Interrupted => return Comment::Quit,
            };
            prompt = COMMENT_PROMPT.to_string();
            let text = line.trim();
            if let Some(mode) = mode_command(text) {
                if mode == self.mode {
                    prompt = format!("Already in {} mode.{COMMENT_PROMPT}", mode_name(mode));
                    continue;
                }
                self.mode = mode;
                println!("Switched to {} mode.", mode_name(mode));
                return Comment::Text(TEMPORARY_INTERRUPTION.into());
            }
            match text {
                "/q" => return Comment::Quit,
                "/h" => print_comment_help(self.mode),
                "" => return Comment::Text(TEMPORARY_INTERRUPTION.into()),
                _ if text == "/compact" || text.starts_with("/compact ") => return Comment::Compact(text["/compact".len()..].trim().to_string()),
                _ => return Comment::Text(text.to_string()),
            }
        }
    }

    /// Returns true when an action matches the whitelist.
    /// @param action - the action
    fn is_whitelisted(&self, action: &Action) -> bool {
        matches!(action, Action::Bash { command, .. } if self.whitelist.iter().any(|r| r.is_match(command)))
    }
}

/// Reads `/y`, `/c` or `/u`.
/// @param text - what the person typed
fn mode_command(text: &str) -> Option<Mode> {
    match text {
        "/y" => Some(Mode::Yolo),
        "/c" => Some(Mode::Confirm),
        "/u" => Some(Mode::Human),
        _ => None,
    }
}

/// The name mini-swe-agent prints for a mode.
/// @param mode - the mode
fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Yolo => "yolo",
        Mode::Confirm => "confirm",
        Mode::Human => "human",
    }
}

/// Prints the comment prompt's commands.
/// @param mode - the current mode
fn print_comment_help(mode: Mode) {
    println!("  Current mode: {}\n  text            send it to the model and carry on\n  Enter           carry on\n  /y              switch to yolo mode (run commands without asking)\n  /c              switch to confirm mode (ask before running commands)\n  /u              switch to human mode (you type the commands)\n  /compact [focus] compact the history now\n  /q or Ctrl+C    end the run; it can be resumed with umbra --resume", mode_name(mode));
}

/// Prints the confirm mode commands.
fn print_help() {
    println!("  Enter  run the commands\n  /y     switch to yolo mode and run\n  /c     confirm mode\n  /u     human mode: you type the commands\n  text   reject the commands and send the text to the model");
}

/// Prints the model's reply.
/// @param step - the step number
/// @param cost - the run's cost so far
/// @param content - the reply text
/// @param actions - the actions it asked for
pub fn show_assistant(step: u32, cost: f64, content: &str, actions: &[Action]) {
    if is_quiet() {
        return;
    }
    heading(&format!("assistant (step {step}, ${cost:.4})"), "1;36");
    if !content.trim().is_empty() {
        println!("{}", clip(content.trim(), 4000));
    }
    for action in actions {
        println!("{} {}", paint("1;35", "▶"), paint("35", &clip(&action.describe(), 2000)));
    }
}

/// Prints an action's result.
/// @param observation - the result
pub fn show_observation(observation: &Observation) {
    if is_quiet() {
        return;
    }
    let code = if observation.returncode == 0 { "32" } else { "31" };
    heading(&format!("output (exit {})", observation.returncode), code);
    if !observation.exception_info.is_empty() {
        println!("{}", paint("31", &observation.exception_info));
    }
    println!("{}", clip(observation.output.trim_end(), 2000));
}

/// Prints a notice line.
/// @param label - the heading
/// @param text - the notice
pub fn show_notice(label: &str, text: &str) {
    if is_quiet() {
        return;
    }
    heading(label, "1;33");
    println!("{}", clip(text, 2000));
}

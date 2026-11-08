//! The person at the keyboard: confirming actions, typing commands in human mode, giving a
//! follow up task when the agent finishes, and printing the run as it happens.

use regex::Regex;

use crate::models::action::{Action, Observation};
use crate::models::config::Mode;
use crate::utils::console::{clip, heading, is_quiet, paint, read_line, stdin_is_terminal};

/// What the person decided about a set of actions.
pub enum Decision {
    /// Run them.
    Run,
    /// Do not run them, and tell the model this.
    Reject(String),
}

/// What the person typed in human mode.
pub enum HumanInput {
    Command(String),
    /// Hand control back to the model in the given mode.
    SwitchMode(Mode),
    Quit,
}

/// The interactive state of a run.
pub struct Interaction {
    pub mode: Mode,
    whitelist: Vec<Regex>,
    pub confirm_exit: bool,
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
        Interaction { mode, whitelist, confirm_exit, interactive }
    }

    /// Asks the person to approve actions in confirm mode. Whitelisted commands, yolo mode and a
    /// run with no terminal never ask.
    /// @param actions - the actions the model wants to run
    pub fn confirm(&mut self, actions: &[Action]) -> Decision {
        if self.mode != Mode::Confirm || !self.interactive || actions.iter().all(|a| self.is_whitelisted(a)) {
            return Decision::Run;
        }
        loop {
            let answer = read_line("Run? [Enter] yes, /y yolo, /u human mode, /h help, or type why not: ").unwrap_or_default();
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
            let Some(line) = read_line("command (/c confirm mode, /y yolo mode, /q quit): ") else { return HumanInput::Quit };
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

    /// When the agent finishes, asks for a follow up task. Returns None to end the run.
    pub fn follow_up_task(&self) -> Option<String> {
        if !self.confirm_exit || !self.interactive {
            return None;
        }
        let line = read_line("Agent wants to finish. Type a new task, or press Enter to quit: ")?;
        let line = line.trim();
        if line.is_empty() { None } else { Some(line.to_string()) }
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

    /// Returns true when an action matches the whitelist.
    /// @param action - the action
    fn is_whitelisted(&self, action: &Action) -> bool {
        matches!(action, Action::Bash { command, .. } if self.whitelist.iter().any(|r| r.is_match(command)))
    }
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

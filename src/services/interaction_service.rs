//! The person at the keyboard: confirming actions, typing commands in human mode, giving a
//! follow up task when the agent finishes, and printing the run as it happens.

use regex::Regex;

use crate::models::action::{Action, Observation};
use crate::models::config::Mode;
use super::agent_service::REPLIED_STATUS;
use crate::ui::block::Block;
use crate::utils::console::{clip, emit, is_quiet, is_verbose, paint, prompt_line, read_line, stdin_is_terminal, stdout_is_terminal, terminal_width, Input};
use crate::utils::screen;

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

/// The hint above the follow up prompt.
pub const FOLLOW_UP: &str = "  Type a follow up task, or press Enter to quit";
/// The hint above the follow up prompt when an empty line does not end the run.
pub const FOLLOW_UP_KEEP_OPEN: &str = "  Type a follow up task";

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

    /// Builds the settings for a run with nobody at a terminal, such as an ACP session, without
    /// looking at stdin. On Windows, asking whether stdin is a terminal waits while another thread
    /// is reading the same pipe, which is what the ACP server's reader thread is always doing.
    /// @param mode - the mode; nothing is ever asked
    pub fn detached(mode: Mode) -> Self {
        Interaction { mode, whitelist: Vec::new(), confirm_exit: false, keep_open: false, interactive: false }
    }

    /// Asks the person to approve actions in confirm mode. Whitelisted commands, yolo mode and a
    /// run with no terminal never ask.
    /// @param actions - the actions the model wants to run
    pub fn confirm(&mut self, actions: &[Action]) -> Decision {
        if self.mode != Mode::Confirm || !self.interactive || actions.iter().all(|a| self.is_whitelisted(a)) {
            return Decision::Run;
        }
        loop {
            show_pending(actions);
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
        let hint = if self.keep_open { FOLLOW_UP_KEEP_OPEN } else { FOLLOW_UP };
        loop {
            let line = match prompt_with_hint(hint) {
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
                emit(&format!("Switched to {} mode.", mode_name(mode)));
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

/// Shows the commands waiting for approval above the input in the fullscreen view, in full.
/// @param actions - the actions the model wants to run
fn show_pending(actions: &[Action]) {
    if !screen::is_active() {
        return;
    }
    let lines = actions.iter().flat_map(|action| {
        let text = match action {
            Action::Bash { command, .. } => format!("$ {}", command.trim()),
            Action::Mcp { .. } => format!("→ {}", action.describe()),
        };
        crate::ui::markdown::sanitize(&text).lines().map(|l| format!("  {l}")).collect::<Vec<_>>()
    }).collect();
    screen::set_pending(lines);
}

/// Prints the comment prompt's commands.
/// @param mode - the current mode
fn print_comment_help(mode: Mode) {
    emit(&format!("  Current mode: {}\n  text            send it to the model and carry on\n  Enter           carry on\n  /y              switch to yolo mode (run commands without asking)\n  /c              switch to confirm mode (ask before running commands)\n  /u              switch to human mode (you type the commands)\n  /compact [focus] compact the history now\n  /q or Ctrl+C    end the run; umbra resume continues it later", mode_name(mode)));
}

/// Prints the confirm mode commands.
fn print_help() {
    emit("  Enter  run the commands\n  /y     switch to yolo mode and run\n  /c     confirm mode\n  /u     human mode: you type the commands\n  text   reject the commands and send the text to the model");
}

/// How much attention a run event asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Routine: one dim line.
    Info,
    /// Something the person may want to act on: yellow.
    Warning,
    /// Something went wrong: red.
    Failure,
}

/// The color codes of the run's output.
const CYAN: &str = "36";
const MAGENTA: &str = "35";
const GREEN: &str = "32";
const YELLOW: &str = "33";
const RED: &str = "31";
const DIM: &str = "90";

/// The command output lines shown when the output is not verbose.
const OUTPUT_LINES: usize = 10;
/// The longest output line shown when the output is not verbose.
const OUTPUT_LINE_CHARS: usize = 400;

/// Prints the model's reply: its text after a `●`, then each action. In verbose mode a dim line
/// with the step number and the cost comes first.
/// @param step - the step number
/// @param cost - the run's cost so far
/// @param content - the reply text
/// @param actions - the actions it asked for
pub fn show_assistant(step: u32, cost: f64, content: &str, actions: &[Action]) {
    if is_quiet() {
        return;
    }
    if screen::is_active() {
        // The actions become runs of a command group through the view's event hook.
        show_reply_text(content);
        return;
    }
    if is_verbose() {
        emit(&paint(DIM, &format!("\n  step {step} · ${cost:.4}")));
    }
    show_reply_text(content);
    for action in actions {
        show_action(action);
    }
}

/// Prints the text of a reply after a `●`, indented to line up under it. A blank line comes
/// first, so each reply starts a new block.
/// @param content - the reply text
pub fn show_reply_text(content: &str) {
    if is_quiet() {
        return;
    }
    let text = content.trim();
    if screen::is_active() {
        if !text.is_empty() {
            screen::push_block(Block::Reply { markdown: clip(text, 20_000) });
        }
        return;
    }
    if !is_verbose() {
        emit("");
    }
    if text.is_empty() {
        return;
    }
    // A reply is often the final answer of a turn, so it is shown nearly whole. At a terminal it
    // is rendered from markdown; a pipe gets the source as the model wrote it.
    let text = clip(text, 20_000);
    if stdout_is_terminal() {
        let rows = crate::ui::markdown::render(&text, terminal_width().saturating_sub(3).max(20), crate::utils::console::color_enabled());
        let body: Vec<String> = rows.iter().enumerate().map(|(i, row)| if i == 0 { format!("{} {}", paint(CYAN, "●"), row.text) } else { format!("  {}", row.text) }).collect();
        emit(&body.join("\n"));
        return;
    }
    emit(&format!("{} {}", paint(CYAN, "●"), indent_after_first(&text, "  ")));
}

/// Prints one action: `$ command` for a shell command, `→ server.tool arguments` for an MCP tool.
/// @param action - the action
fn show_action(action: &Action) {
    let line = match action {
        Action::Bash { command, .. } if !is_verbose() && command.contains(SUBMIT_MARKER) => {
            let limit = if stdout_is_terminal() { terminal_width().saturating_sub(6).clamp(20, 80) } else { 80 };
            emit(&format!("  {}", paint(DIM, &format!("$ {}", first_line_cut(command.trim(), limit)))));
            return;
        }
        Action::Bash { command, .. } => format!("$ {}", clip(command.trim(), 2000)),
        Action::Mcp { .. } if is_verbose() => format!("→ {}", clip(&action.describe(), 2000)),
        Action::Mcp { server, tool, arguments, .. } => format!("→ {server}.{tool} {}", clip(&arguments.to_string(), 200)),
    };
    emit(&format!("  {}", paint(MAGENTA, &indent_after_first(&line, "    "))));
}

/// Prints an action's result under a `⎿` gutter. Without verbose output only the first lines are
/// shown, with a count of the rest. An exit code other than 0 is shown in red.
/// @param observation - the result
pub fn show_observation(observation: &Observation) {
    if is_quiet() || screen::is_active() {
        return;
    }
    let output = observation.output.trim_end();
    let mut lines: Vec<String> = if is_verbose() { clip(output, 2000).lines().map(String::from).collect() } else { short_output(output) };
    if !observation.exception_info.is_empty() {
        lines.extend(observation.exception_info.lines().map(|l| paint(RED, l)));
    }
    if observation.returncode != 0 {
        lines.push(paint(RED, &format!("exit {}", observation.returncode)));
    }
    if lines.is_empty() {
        lines.push(paint(DIM, "(no output)"));
    }
    let gutter = paint(DIM, "⎿");
    let body: Vec<String> = lines.iter().enumerate().map(|(i, line)| if i == 0 { format!("  {gutter}  {line}") } else { format!("     {line}") }).collect();
    emit(&body.join("\n"));
}

/// The first lines of a command's output, each cut to fit the terminal after the gutter, with a
/// dim count of the lines left out.
/// @param output - the whole output
fn short_output(output: &str) -> Vec<String> {
    if output.is_empty() {
        return Vec::new();
    }
    let all: Vec<&str> = output.lines().collect();
    let limit = if stdout_is_terminal() { terminal_width().saturating_sub(7).clamp(20, OUTPUT_LINE_CHARS) } else { OUTPUT_LINE_CHARS };
    let mut lines: Vec<String> = all.iter().take(OUTPUT_LINES).map(|l| cut_line(l, limit)).collect();
    if all.len() > OUTPUT_LINES {
        let more = all.len() - OUTPUT_LINES;
        lines.push(paint(DIM, &format!("… {more} more {} (--verbose shows more)", if more == 1 { "line" } else { "lines" })));
    }
    lines
}

/// Cuts one line to `limit` characters with an ellipsis.
/// @param line - the line
/// @param limit - the most characters to keep
fn cut_line(line: &str, limit: usize) -> String {
    if line.chars().count() <= limit {
        return line.to_string();
    }
    let kept: String = line.chars().take(limit).collect();
    format!("{kept}…")
}

/// The text a command prints to submit. A command that contains it is shown on one dim line,
/// because what it prints is shown as the final output when the turn ends.
const SUBMIT_MARKER: &str = "COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT";

/// The first line of text cut to `limit` characters, with an ellipsis when anything was left out.
/// @param text - the text
/// @param limit - the most characters to keep
fn first_line_cut(text: &str, limit: usize) -> String {
    let first = text.lines().next().unwrap_or("");
    let cut = cut_line(first, limit);
    if cut.len() < text.len() && !cut.ends_with('…') { format!("{cut} …") } else { cut }
}

/// Wraps text at spaces to fit the terminal after an indent, so long lines stay under their
/// marker instead of running back to the first column. Only in a terminal; a pipe gets the text
/// as it is. A word longer than the line is left whole.
/// @param text - the text
/// @param indent - the columns before the text on every line
fn wrap(text: &str, indent: usize) -> String {
    if !stdout_is_terminal() {
        return text.to_string();
    }
    let room = terminal_width().saturating_sub(indent + 1).max(20);
    text.lines().map(|line| wrap_line(line, room)).collect::<Vec<_>>().join("\n")
}

/// Wraps one line at spaces into lines of at most `room` characters.
/// @param line - the line
/// @param room - the most characters on a line
fn wrap_line(line: &str, room: usize) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in line.split(' ') {
        let needed = current.chars().count() + word.chars().count() + usize::from(!current.is_empty());
        if needed > room && !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    out.push(current);
    out.join("\n")
}

/// Indents every line after the first, so wrapped text lines up under a marker.
/// @param text - the text
/// @param pad - the indent
fn indent_after_first(text: &str, pad: &str) -> String {
    text.replace('\n', &format!("\n{pad}"))
}

/// Prints one event of the run. Routine events are one dim line with a marker such as `↻`;
/// warnings are yellow with `▲` and failures red with `✖`.
/// @param tone - how much attention it asks for
/// @param marker - the marker of a routine event
/// @param text - what happened
pub fn show_event(tone: Tone, marker: &str, text: &str) {
    if is_quiet() {
        return;
    }
    let text = clip(text.trim(), 2000);
    if screen::is_active() {
        screen::push_block(Block::Notice { tone, marker: marker.to_string(), text });
        return;
    }
    let line = match tone {
        Tone::Info => paint(DIM, &format!("  {marker} {}", indent_after_first(&wrap(&text, 4), "    "))),
        Tone::Warning => format!("\n{}", paint(YELLOW, &format!("▲ {}", indent_after_first(&wrap(&text, 2), "  ")))),
        Tone::Failure => format!("\n{}", paint(RED, &format!("✖ {}", indent_after_first(&wrap(&text, 2), "  ")))),
    };
    emit(&line);
}

/// Prints what the person typed after a marker: `❯` for a task, `›` for a line sent while the
/// agent worked.
/// @param marker - the marker
/// @param text - what they typed
pub fn show_user_text(marker: &str, text: &str) {
    if is_quiet() {
        return;
    }
    if screen::is_active() {
        screen::push_block(Block::UserPrompt { marker: marker.to_string(), text: clip(text.trim(), 2000) });
        return;
    }
    emit(&format!("\n{} {}", paint("1;33", marker), indent_after_first(&wrap(&clip(text.trim(), 2000), 2), "  ")));
}

/// How a turn ended, for the terminal.
pub struct TurnSummary<'a> {
    pub status: &'a str,
    pub submission: &'a str,
}

/// Prints how a turn ended. A turn that finished its work prints only the final output of the
/// submit style; the reply style's answer is already on screen. Any other ending prints one line
/// saying why the agent stopped. The cost and the time are in the status bar, and no resume
/// command is printed.
/// @param summary - the turn's ending
pub fn show_turn_end(summary: &TurnSummary) {
    if is_quiet() {
        return;
    }
    let (code, headline, show_output) = turn_headline(summary.status, summary.submission);
    let submission = summary.submission.trim();
    let succeeded = summary.status == "Submitted" || summary.status == REPLIED_STATUS;
    if !succeeded {
        let tone = if summary.status == "Interrupted" { Tone::Warning } else if code == DIM { Tone::Info } else { Tone::Failure };
        if screen::is_active() {
            screen::push_block(Block::Notice { tone, marker: "■".into(), text: headline.trim_start_matches(['✖', '▲', '■', ' ']).to_string() });
        } else {
            emit(&format!("\n{}", paint(code, &headline)));
        }
    }
    if show_output && !submission.is_empty() {
        if succeeded {
            show_reply_text(submission);
        } else if screen::is_active() {
            screen::push_block(Block::Text(clip(submission, 4000)));
        } else {
            emit(&format!("  {}", indent_after_first(&wrap(&clip(submission, 4000), 2), "  ")));
        }
    }
}

/// The color, the headline and whether to show the final output, for a turn's exit status.
/// @param status - the exit status
/// @param submission - the final output, or the error for a failed model call
fn turn_headline(status: &str, submission: &str) -> (&'static str, String, bool) {
    match status {
        "Submitted" => (GREEN, "✔ Done".into(), true),
        // The reply that ended the turn is already on screen.
        REPLIED_STATUS => (GREEN, "✔ Done".into(), false),
        "UserQuit" => (DIM, "■ Ended".into(), false),
        "Interrupted" => (YELLOW, "▲ Interrupted".into(), false),
        "LimitsExceeded" => (RED, "✖ Stopped: the step or cost limit was reached".into(), true),
        "TimeExceeded" => (RED, "✖ Stopped: the time limit was reached".into(), true),
        "RepeatedFormatError" => (RED, "✖ Stopped: the model replied without a usable command too many times in a row".into(), true),
        "ModelError" => (RED, format!("✖ Stopped: {}", submission.lines().next().unwrap_or("the model call failed")), false),
        other => (RED, format!("✖ Stopped: {other}"), true),
    }
}

/// Prints a dim hint line, then a `❯` prompt, and reads one line.
/// @param hint - the hint above the prompt
pub fn prompt_with_hint(hint: &str) -> Input {
    if !hint.is_empty() && !screen::is_active() {
        emit(&format!("\n{}", paint(DIM, hint)));
    }
    prompt_line("❯ ")
}

#[cfg(test)]
mod display_tests {
    use super::*;

    #[test]
    fn wraps_at_spaces_and_keeps_long_words_whole() {
        assert_eq!(wrap_line("aaa bbb ccc", 7), "aaa bbb\nccc");
        assert_eq!(wrap_line("a averyveryverylongword b", 6), "a\naveryveryverylongword\nb");
        assert_eq!(wrap_line("short", 40), "short");
    }

    #[test]
    fn a_submit_command_is_cut_to_its_first_line() {
        assert_eq!(first_line_cut("echo X && echo done", 80), "echo X && echo done");
        assert_eq!(first_line_cut("echo X\ncat file", 80), "echo X …");
        assert_eq!(first_line_cut(&"x".repeat(100), 10), format!("{}…", "x".repeat(10)));
    }
}

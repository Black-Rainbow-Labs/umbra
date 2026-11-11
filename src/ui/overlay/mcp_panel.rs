//! The MCP panel: a switch between "this session" and "all sessions", then one row per registered
//! server with a checkbox, its state, its tool count and, for a server that failed, the first line
//! of the error. For this session, Space or a click turns a server on or off in the running
//! session only. For all sessions, the checkbox shows whether the config files let the server
//! start, and Space or a click changes `~/.umbra/config.toml` and the running session together.
//! `s`, Left or Right switches the scope, `r` starts a failed server again, Enter on a failed
//! server shows the whole error and its log file, Esc closes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::{Outcome, Overlay, Placement};
use crate::services::mcp::ServerState;
use crate::services::mcp_settings_service as saved;
use crate::ui::block::{cut_to, paint, LayoutCtx, Row, Target};
use crate::ui::live;
use crate::utils::ansi;
use crate::utils::keys::Key;

/// The target number of the scope switch row.
const SCOPE_ITEM: usize = 10_000;

/// What a checkbox change applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    /// The running session only.
    #[default]
    Session,
    /// Every session, through `~/.umbra/config.toml`, and the running one.
    All,
}

/// The panel.
#[derive(Default)]
pub struct McpPanel {
    selected: usize,
    /// The server whose whole error is shown.
    expanded: Option<String>,
    scope: Scope,
    /// For all sessions: whether the config files let each server start, read when the scope is
    /// switched and after each change.
    saved: BTreeMap<String, bool>,
    /// A line under the title, such as an error from writing the config file.
    message: Option<(String, bool)>,
}

impl McpPanel {
    /// A panel with the first server selected, for this session.
    pub fn new() -> Self {
        McpPanel::default()
    }

    /// The working folder, whose project config file can turn servers off.
    fn cwd() -> PathBuf {
        live::header().map(|h| h.cwd).unwrap_or_default()
    }

    /// The registered servers and their states, in name order.
    fn names() -> Vec<(String, ServerState)> {
        live::mcp().map(|m| m.snapshot()).unwrap_or_default()
    }

    /// Reads again which servers the config files let start.
    fn refresh_saved(&mut self) {
        let settings = saved::saved_settings(&McpPanel::cwd());
        self.saved = McpPanel::names().into_iter().map(|(name, _)| {
            let on = crate::services::mcp::is_selected(&name, &settings);
            (name, on)
        }).collect();
    }

    /// Switches between this session and all sessions.
    fn switch_scope(&mut self) {
        self.scope = match self.scope {
            Scope::Session => Scope::All,
            Scope::All => Scope::Session,
        };
        self.message = None;
        if self.scope == Scope::All {
            self.refresh_saved();
        }
    }

    /// Whether a server's checkbox is ticked in the current scope.
    /// @param name - the server
    /// @param state - its state in this session
    fn checked(&self, name: &str, state: &ServerState) -> bool {
        match self.scope {
            Scope::Session => !matches!(state, ServerState::Off),
            Scope::All => self.saved.get(name).copied().unwrap_or(true),
        }
    }

    /// Flips the selected server's checkbox. For all sessions the config file is written now and
    /// the running session follows once the lock is released.
    fn toggle(&mut self) -> Outcome {
        let servers = McpPanel::names();
        let Some((name, state)) = servers.get(self.selected).cloned() else { return Outcome::Keep };
        let on = !self.checked(&name, &state);
        if self.scope == Scope::All {
            if let Err(error) = saved::set_saved(&saved::user_file(), &name, on) {
                self.message = Some((format!("not saved: {error:#}"), true));
                return Outcome::Keep;
            }
            self.refresh_saved();
            self.message = Some(match on && saved::project_turns_off(&McpPanel::cwd(), &name) {
                true => (format!("{name} is on in {}, but the project file still turns it off", saved::user_file().display()), true),
                false => (format!("{name} {} for every session in {}", if on { "on" } else { "off" }, saved::user_file().display()), false),
            });
        }
        let running_on = !matches!(state, ServerState::Off);
        if running_on == on {
            return Outcome::Keep;
        }
        Outcome::Effects(vec![Box::new(move || {
            if let Some(manager) = live::mcp() {
                manager.set_enabled(&name, on);
            }
        })])
    }

    /// Starts the selected server again when it failed.
    fn restart(&mut self) -> Outcome {
        let servers = McpPanel::names();
        let Some((name, ServerState::Failed { .. })) = servers.get(self.selected).cloned() else { return Outcome::Keep };
        Outcome::Effects(vec![Box::new(move || {
            if let Some(manager) = live::mcp() {
                manager.restart(&name);
            }
        })])
    }

    /// The scope switch row: the two choices with the current one filled in.
    /// @param width - the columns available
    /// @param ctx - the frame
    fn scope_row(&self, width: usize, ctx: &LayoutCtx) -> String {
        let choice = |label: &str, on: bool| if on { paint(ctx, "1", &format!("(•) {label}")) } else { paint(ctx, "90", &format!("( ) {label}")) };
        let text = format!("   applies to  {}   {}", choice("this session", self.scope == Scope::Session), choice("all sessions", self.scope == Scope::All));
        ansi::fit(&text, width)
    }

    /// One server's row: the checkbox, the name, the state and the tool count or error.
    /// @param name - the server name
    /// @param state - its state
    /// @param selected - true for the selected row
    /// @param width - the columns available
    /// @param ctx - the frame
    fn server_row(&self, name: &str, state: &ServerState, selected: bool, width: usize, ctx: &LayoutCtx) -> String {
        let toggle = if self.checked(name, state) { "[x]" } else { "[ ]" };
        let (word, mut detail, code) = match state {
            ServerState::Off => ("off".to_string(), String::new(), "90"),
            ServerState::Starting { since } => (format!("{} starting", ctx.spinner), format!("{}s", since.elapsed().as_secs()), "36"),
            ServerState::Ready { tools, .. } => ("running".to_string(), format!("{} {}", tools.len(), if tools.len() == 1 { "tool" } else { "tools" }), "32"),
            ServerState::Failed { error, at, .. } => ("✖ failed".to_string(), format!("{} ({}s ago)", error.lines().next().unwrap_or(""), at.elapsed().as_secs()), "31"),
        };
        if self.scope == Scope::All && saved::project_turns_off(&McpPanel::cwd(), name) {
            detail = format!("off in the project file  {detail}");
        }
        let pointer = if selected { "›" } else { " " };
        let left = format!(" {pointer} {toggle} {name:<16} ");
        let state_text = format!("{word:<11}");
        let room = width.saturating_sub(left.chars().count() + state_text.chars().count() + 1);
        let text = format!("{left}{}{}", paint(ctx, code, &state_text), paint(ctx, "90", &cut_to(&crate::ui::markdown::sanitize(&detail), room)));
        let text = ansi::fit(&text, width);
        if selected { paint(ctx, "1", &text) } else { text }
    }

    /// The whole error and the log file of an expanded failed server.
    /// @param error - the error
    /// @param log_path - the server's log file
    /// @param width - the columns available
    /// @param ctx - the frame
    fn error_rows(error: &str, log_path: &std::path::Path, width: usize, ctx: &LayoutCtx) -> Vec<Row> {
        let mut rows: Vec<Row> = crate::ui::block::word_wrap(&crate::ui::markdown::sanitize(error), width.saturating_sub(8)).into_iter().map(|(line, _)| Row::text(paint(ctx, "31", &format!("        {line}")))).collect();
        rows.push(Row::text(paint(ctx, "90", &format!("        log {}", log_path.display()))));
        rows
    }
}

impl Overlay for McpPanel {
    /// The panel is drawn above the input line.
    fn placement(&self) -> Placement {
        Placement::AboveStatus
    }

    /// The title row, the scope switch, then one row per server, then the whole error of an
    /// expanded failed server.
    /// @param width - the columns available
    /// @param _height - the most rows it may take
    /// @param ctx - the frame
    fn rows(&mut self, width: usize, _height: usize, ctx: &LayoutCtx) -> Vec<Row> {
        let servers = McpPanel::names();
        self.selected = self.selected.min(servers.len().saturating_sub(1));
        let hint = "Space toggles · s switches scope · r restarts · Esc closes";
        let pad = width.saturating_sub(" MCP servers".len() + hint.chars().count() + 1);
        let mut rows = vec![Row::text(ansi::fit(&format!("{}{}{}", paint(ctx, "1", " MCP servers"), " ".repeat(pad.max(1)), paint(ctx, "90", hint)), width))];
        rows.push(Row { text: self.scope_row(width, ctx), target: Some(Target::Item(SCOPE_ITEM)), ..Default::default() });
        if let Some((text, error)) = &self.message {
            rows.push(Row::text(ansi::fit(&paint(ctx, if *error { "31" } else { "90" }, &format!("   {text}")), width)));
        }
        if servers.is_empty() {
            rows.push(Row::text(paint(ctx, "90", "   No MCP server is defined. `umbra mcp` lists where Umbra looks.")));
        }
        for (index, (name, state)) in servers.iter().enumerate() {
            rows.push(Row { text: self.server_row(name, state, index == self.selected, width, ctx), target: Some(Target::Item(index)), ..Default::default() });
            if let (Some(open), ServerState::Failed { error, log_path, .. }) = (&self.expanded, state) {
                if open == name {
                    rows.extend(McpPanel::error_rows(error, log_path, width, ctx));
                }
            }
        }
        rows
    }

    /// Up and Down move, Space toggles, `s`, Left and Right switch the scope, `r` restarts,
    /// Enter expands a failed server, Esc closes.
    /// @param key - the key
    fn key(&mut self, key: &Key) -> Outcome {
        let count = McpPanel::names().len();
        match key {
            Key::Up | Key::BackTab => self.selected = self.selected.saturating_sub(1),
            Key::Down | Key::Tab => self.selected = (self.selected + 1).min(count.saturating_sub(1)),
            Key::Text(text) if text == " " => return self.toggle(),
            Key::Text(text) if text == "s" => self.switch_scope(),
            Key::Left | Key::Right => self.switch_scope(),
            Key::Text(text) if text == "r" => return self.restart(),
            Key::Enter => {
                let servers = McpPanel::names();
                if let Some((name, ServerState::Failed { .. })) = servers.get(self.selected) {
                    self.expanded = if self.expanded.as_deref() == Some(name.as_str()) { None } else { Some(name.clone()) };
                }
            }
            Key::Escape => return Outcome::Close,
            _ => {}
        }
        Outcome::Keep
    }

    /// A click on the scope row switches the scope; a click on a server's row selects it and
    /// toggles it.
    /// @param item - the row's number
    fn click(&mut self, item: usize) -> Outcome {
        if item == SCOPE_ITEM {
            self.switch_scope();
            return Outcome::Keep;
        }
        self.selected = item;
        self.toggle()
    }

    /// The spinner moves while a server starts.
    fn animating(&self) -> bool {
        live::mcp().is_some_and(|m| m.any_starting())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scope_switches_and_the_row_shows_it() {
        let mut panel = McpPanel::new();
        let ctx = LayoutCtx { color: false, now: std::time::Instant::now(), spinner: '⠋' };
        assert!(panel.scope_row(80, &ctx).contains("(•) this session"));
        panel.key(&Key::Text("s".into()));
        assert_eq!(panel.scope, Scope::All);
        assert!(panel.scope_row(80, &ctx).contains("(•) all sessions"));
        panel.click(SCOPE_ITEM);
        assert_eq!(panel.scope, Scope::Session);
    }
}

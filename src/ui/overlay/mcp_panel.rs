//! The MCP panel: one row per registered server with a toggle, its state, its tool count and, for
//! a server that failed, the first line of the error. Space or a click turns a server on or off
//! for this session, `r` starts a failed server again, Enter on a failed server shows the whole
//! error and its log file, Esc closes.

use super::{Outcome, Overlay, Placement};
use crate::services::mcp::ServerState;
use crate::ui::block::{cut_to, paint, LayoutCtx, Row, Target};
use crate::ui::live;
use crate::utils::ansi;
use crate::utils::keys::Key;

/// The panel.
#[derive(Default)]
pub struct McpPanel {
    selected: usize,
    /// The server whose whole error is shown.
    expanded: Option<String>,
}

impl McpPanel {
    /// A panel with the first server selected.
    pub fn new() -> Self {
        McpPanel::default()
    }

    /// The names of the registered servers, in name order.
    fn names() -> Vec<(String, ServerState)> {
        live::mcp().map(|m| m.snapshot()).unwrap_or_default()
    }

    /// Turns the selected server on when it is off or failed, and off otherwise.
    fn toggle(&mut self) -> Outcome {
        let servers = McpPanel::names();
        let Some((name, state)) = servers.get(self.selected).cloned() else { return Outcome::Keep };
        let on = matches!(state, ServerState::Off);
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

    /// One server's row: the toggle, the name, the state and the tool count or error.
    /// @param name - the server name
    /// @param state - its state
    /// @param selected - true for the selected row
    /// @param width - the columns available
    /// @param ctx - the frame
    fn server_row(name: &str, state: &ServerState, selected: bool, width: usize, ctx: &LayoutCtx) -> String {
        let toggle = if matches!(state, ServerState::Off) { "[ ]" } else { "[x]" };
        let (word, detail, code) = match state {
            ServerState::Off => ("off".to_string(), String::new(), "90"),
            ServerState::Starting { since } => (format!("{} starting", ctx.spinner), format!("{}s", since.elapsed().as_secs()), "36"),
            ServerState::Ready { tools, .. } => ("running".to_string(), format!("{} {}", tools.len(), if tools.len() == 1 { "tool" } else { "tools" }), "32"),
            ServerState::Failed { error, at, .. } => ("✖ failed".to_string(), format!("{} ({}s ago)", error.lines().next().unwrap_or(""), at.elapsed().as_secs()), "31"),
        };
        let pointer = if selected { "›" } else { " " };
        let left = format!(" {pointer} {toggle} {name:<16} ");
        let state_text = format!("{word:<11}");
        let room = width.saturating_sub(left.chars().count() + state_text.chars().count() + 1);
        let text = format!("{left}{}{}", paint(ctx, code, &state_text), paint(ctx, "90", &cut_to(&crate::ui::markdown::sanitize(&detail), room)));
        let text = ansi::fit(&text, width);
        if selected { paint(ctx, "1", &text) } else { text }
    }
}

impl Overlay for McpPanel {
    /// The panel is drawn above the status bar.
    fn placement(&self) -> Placement {
        Placement::AboveStatus
    }

    /// The title row, then one row per server, then the whole error of an expanded failed server.
    /// @param width - the columns available
    /// @param _height - the most rows it may take
    /// @param ctx - the frame
    fn rows(&mut self, width: usize, _height: usize, ctx: &LayoutCtx) -> Vec<Row> {
        let servers = McpPanel::names();
        self.selected = self.selected.min(servers.len().saturating_sub(1));
        let hint = "Space toggles · r restarts · Enter shows the error · Esc closes";
        let pad = width.saturating_sub(" MCP servers".len() + hint.len() + 1);
        let mut rows = vec![Row::text(ansi::fit(&format!("{}{}{}", paint(ctx, "1", " MCP servers"), " ".repeat(pad.max(1)), paint(ctx, "90", hint)), width))];
        if servers.is_empty() {
            rows.push(Row::text(paint(ctx, "90", "   No MCP server is defined. `umbra mcp` lists where Umbra looks.")));
        }
        for (index, (name, state)) in servers.iter().enumerate() {
            rows.push(Row { text: McpPanel::server_row(name, state, index == self.selected, width, ctx), target: Some(Target::Item(index)), ..Default::default() });
            if let (Some(open), ServerState::Failed { error, log_path, .. }) = (&self.expanded, state) {
                if open == name {
                    for (line, _) in crate::ui::block::word_wrap(&crate::ui::markdown::sanitize(error), width.saturating_sub(8)) {
                        rows.push(Row::text(paint(ctx, "31", &format!("        {line}"))));
                    }
                    rows.push(Row::text(paint(ctx, "90", &format!("        log {}", log_path.display()))));
                }
            }
        }
        rows
    }

    /// Up and Down move, Space toggles, `r` restarts, Enter expands a failed server, Esc closes.
    /// @param key - the key
    fn key(&mut self, key: &Key) -> Outcome {
        let count = McpPanel::names().len();
        match key {
            Key::Up | Key::BackTab => self.selected = self.selected.saturating_sub(1),
            Key::Down | Key::Tab => self.selected = (self.selected + 1).min(count.saturating_sub(1)),
            Key::Text(text) if text == " " => return self.toggle(),
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

    /// A click on a server's row selects it and toggles it.
    /// @param item - the server's position
    fn click(&mut self, item: usize) -> Outcome {
        self.selected = item;
        self.toggle()
    }

    /// The spinner moves while a server starts.
    fn animating(&self) -> bool {
        live::mcp().is_some_and(|m| m.any_starting())
    }
}

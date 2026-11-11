//! The header block at the top of the fullscreen transcript: the logo and the rows beside it, built
//! from live state at every frame, so the mcp row follows the servers as they start and the model
//! row follows a model switch. The model and mcp rows are click and focus targets.

use std::time::Instant;

use super::block::{LayoutCtx, Row, Target};
use super::live;
use crate::services::mcp::{summary, McpManager};
use crate::utils::ansi;
use crate::utils::banner::{render_banner_step, BannerInfo, REVEAL_STEPS, STEP_MS};
use crate::utils::screen::visible_width;

/// The color of a failed part of the mcp row.
const RED: &str = "31";

/// The mcp row's pieces after its label, from the live servers.
/// @param enabled - false when MCP is turned off
/// @param manager - the session's servers, if any
/// @param spinner - the spinner character for this frame
pub fn mcp_pieces(enabled: bool, manager: Option<&McpManager>, spinner: char) -> Vec<(&'static str, String)> {
    if !enabled {
        return vec![("", "off".into())];
    }
    let states = manager.map(McpManager::snapshot).unwrap_or_default();
    let mut pieces = Vec::new();
    for (index, (text, failed)) in summary(&states, spinner).into_iter().enumerate() {
        if index > 0 {
            pieces.push(("90", " · ".to_string()));
        }
        pieces.push((if failed { RED } else { "" }, text));
    }
    pieces
}

/// The step of the logo's stripe reveal at a moment.
/// @param from - when the reveal started, or None for the finished logo
/// @param now - the frame's time
pub fn reveal_step(from: Option<Instant>, now: Instant) -> usize {
    match from {
        Some(start) => ((now.saturating_duration_since(start).as_millis() / STEP_MS as u128) as usize).min(REVEAL_STEPS),
        None => REVEAL_STEPS,
    }
}

/// True while the header changes with time: the logo is being revealed or a server is starting.
/// @param now - the frame's time
pub fn animating(now: Instant) -> bool {
    let Some(info) = live::header() else { return false };
    reveal_step(info.reveal_from, now) < REVEAL_STEPS || live::mcp().is_some_and(|m| m.any_starting())
}

/// The header's rows at a width, with the model and mcp rows marked as targets.
/// @param width - the screen width
/// @param ctx - the frame
pub fn header_rows(width: usize, ctx: &LayoutCtx) -> Vec<Row> {
    let Some(info) = live::header() else { return Vec::new() };
    let manager = live::mcp();
    let banner = BannerInfo { version: info.version.clone(), model: info.model.clone(), folder: info.folder.clone(), mode: info.mode.clone(), skills: info.skills, mcp: mcp_pieces(info.mcp_enabled, manager.as_deref(), ctx.spinner), model_menu: true };
    let step = if ctx.color { reveal_step(info.reveal_from, ctx.now) } else { REVEAL_STEPS };
    render_banner_step(&banner, width, ctx.color, step).into_iter().map(|text| target_row(text)).collect()
}

/// A header row with its target when it is the model or the mcp row. The focus marker goes two
/// columns before the label, where the layout always leaves space.
/// @param text - the painted row
fn target_row(text: String) -> Row {
    let plain = ansi::plain(&text);
    for (label, target) in [("model   ", Target::HeaderModel), ("mcp     ", Target::HeaderMcp)] {
        if let Some(at) = plain.find(label) {
            let column = visible_width(&plain[..at]);
            let focus_col = (column >= 2 && plain[..at].ends_with("  ")).then(|| column - 2);
            return Row { text, target: Some(target), focus_col, ..Default::default() };
        }
    }
    Row { text, ..Default::default() }
}

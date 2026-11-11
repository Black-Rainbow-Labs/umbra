//! Things drawn over the transcript that take the keyboard while they are open: pickers, the MCP
//! panel, the session picker and the config modal. Each one lays out its own rows, handles keys
//! and clicks on its rows, and says what should happen next.

pub mod config_modal;
pub mod mcp_panel;
pub mod pickers;
pub mod picker;
pub mod update_prompt;

use super::block::{LayoutCtx, Row};
use crate::utils::keys::Key;

/// Work to do after the view's lock is released, because it calls code that draws the screen
/// again, such as turning an MCP server off.
pub type Effect = Box<dyn FnOnce() + Send>;

/// Where an overlay is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// In the rows directly above the status bar.
    AboveStatus,
    /// Centered over the transcript.
    Centered,
}

/// What an overlay asks for after a key or a click.
pub enum Outcome {
    /// Stay open.
    Keep,
    /// Close.
    Close,
    /// Stay open and run this work once the lock is released.
    Effects(Vec<Effect>),
    /// Close and run this work once the lock is released.
    CloseWith(Vec<Effect>),
    /// Close and send this line, as if the person typed it and pressed Enter.
    Line(String),
}

/// An overlay.
pub trait Overlay: Send {
    /// Where it is drawn.
    fn placement(&self) -> Placement;

    /// Its rows at a width, with click targets of the form `Target::Item(n)`. For an overlay
    /// above the status bar, `height` is the most rows it may take; for a centered one it is the
    /// screen height.
    /// @param width - the columns available
    /// @param height - the rows available
    /// @param ctx - the frame
    fn rows(&mut self, width: usize, height: usize, ctx: &LayoutCtx) -> Vec<Row>;

    /// Handles a key.
    /// @param key - the key
    fn key(&mut self, key: &Key) -> Outcome;

    /// Handles a click on one of its rows.
    /// @param item - the `n` of the row's `Target::Item(n)`
    fn click(&mut self, item: usize) -> Outcome;

    /// True while it draws something that changes with time, such as a spinner.
    fn animating(&self) -> bool {
        false
    }

    /// True when typed text closes it and goes to the input line, for a prompt that must never
    /// take a message the person is typing.
    fn passes_typing(&self) -> bool {
        false
    }
}

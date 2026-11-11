//! The configuration modal. Built in a later step; for now it opens and closes.

use super::{Outcome, Overlay, Placement};
use crate::ui::block::{LayoutCtx, Row};
use crate::utils::keys::Key;

/// The configuration modal.
#[derive(Default)]
pub struct ConfigModal;

impl ConfigModal {
    /// Opens the modal.
    pub fn open() -> Box<dyn Overlay> {
        Box::new(ConfigModal)
    }
}

impl Overlay for ConfigModal {
    /// Centered over the transcript.
    fn placement(&self) -> Placement {
        Placement::Centered
    }

    /// One row.
    /// @param _width - the columns available
    /// @param _height - the rows available
    /// @param _ctx - the frame
    fn rows(&mut self, _width: usize, _height: usize, _ctx: &LayoutCtx) -> Vec<Row> {
        vec![Row::text(" Configure Umbra")]
    }

    /// Esc closes.
    /// @param key - the key
    fn key(&mut self, key: &Key) -> Outcome {
        if *key == Key::Escape { Outcome::Close } else { Outcome::Keep }
    }

    /// Nothing to click.
    /// @param _item - the item
    fn click(&mut self, _item: usize) -> Outcome {
        Outcome::Keep
    }
}

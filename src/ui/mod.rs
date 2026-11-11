//! The fullscreen view's interface: the transcript of blocks, the header, the overlays, mouse
//! selection, and the frame that draws them. `utils::screen` owns the terminal and hands keys to
//! the view; the agent and the run add blocks through it.

pub mod actions;
pub mod block;
pub mod frame;
pub mod header;
pub mod live;
pub mod markdown;
pub mod overlay;
pub mod rebuild;
pub mod selection;
pub mod transcript;
pub mod view;

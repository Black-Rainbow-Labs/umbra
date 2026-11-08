//! The record of one compaction, kept in the trajectory's `info.compactions` list so the file
//! shows what was removed from the model's view and why.

use serde::Serialize;

use super::message::Message;

/// What a compaction did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CompactionKind {
    /// Old command outputs were replaced by a placeholder.
    Mask,
    /// The middle of the history was replaced by a summary the model wrote.
    Summary,
}

/// Why a compaction ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CompactionReason {
    /// The estimated request size passed the trigger.
    Threshold,
    /// The provider refused a request as too long.
    Overflow,
    /// The person typed `/compact`.
    Manual,
}

/// One compaction.
#[derive(Debug, Clone, Serialize)]
pub struct CompactionRecord {
    pub kind: CompactionKind,
    pub reason: CompactionReason,
    /// The number of model calls made before this compaction.
    pub step: u32,
    pub tokens_before: u64,
    pub tokens_after: u64,
    pub window: u64,
    /// Mask only: the positions in the history whose content was replaced.
    pub masked_message_indexes: Vec<usize>,
    /// Summary only: the messages the summary replaced, in full.
    pub removed_messages: Vec<Message>,
    /// Summary only: the text the model wrote.
    pub summary: String,
    /// The cost of the summary call, 0 for a mask.
    pub cost: f64,
}

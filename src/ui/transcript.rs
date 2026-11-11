//! The transcript: the list of blocks, each with an id that stays the same while it is on screen,
//! and the rows each one was last laid out to, so a frame only lays out a block again when its
//! content, its expand state or the width changed.

use std::collections::VecDeque;

use super::block::{Block, CommandGroup, CommandRun, LayoutCtx, Row};

/// The most rows the transcript keeps, counted at the last width. Older blocks are dropped.
pub const MAX_ROWS: usize = 20_000;

/// One block and its cached layout.
pub struct Entry {
    pub id: u64,
    pub block: Block,
    /// The width and rows of the last layout, when it is still valid.
    cache: Option<(usize, Vec<Row>)>,
}

/// The blocks of the transcript, oldest first.
#[derive(Default)]
pub struct Transcript {
    entries: VecDeque<Entry>,
    next_id: u64,
    /// The command group new runs join, until a reply or a prompt closes it.
    open_group: Option<u64>,
    /// The rows counted at the last width, for the cap.
    counted: usize,
}

impl Transcript {
    /// Adds a block at the end and returns its id. A block that is not a command group closes
    /// the open group, so the order on screen matches the order of events.
    /// @param block - the block
    pub fn push(&mut self, block: Block) -> u64 {
        if !matches!(block, Block::CommandGroup(_)) {
            self.open_group = None;
        }
        self.next_id += 1;
        let id = self.next_id;
        self.entries.push_back(Entry { id, block, cache: None });
        id
    }

    /// Ends the open command group, so the next run starts a new one.
    pub fn close_group(&mut self) {
        self.open_group = None;
    }

    /// Adds a run to the open command group, or to a new group, and returns the group id and the
    /// run's position in it.
    /// @param run - the run
    pub fn add_run(&mut self, run: CommandRun) -> (u64, usize) {
        if let Some(id) = self.open_group {
            if let Some(Block::CommandGroup(group)) = self.block_mut(id) {
                group.runs.push(run);
                let index = group.runs.len() - 1;
                return (id, index);
            }
        }
        let id = self.push(Block::CommandGroup(CommandGroup { runs: vec![run], expanded: false }));
        self.open_group = Some(id);
        (id, 0)
    }

    /// The block with an id, for a change. Its cached layout is dropped.
    /// @param id - the block id
    pub fn block_mut(&mut self, id: u64) -> Option<&mut Block> {
        let entry = self.entries.iter_mut().find(|e| e.id == id)?;
        entry.cache = None;
        Some(&mut entry.block)
    }

    /// The block with an id.
    /// @param id - the block id
    #[cfg(test)]
    pub fn block(&self, id: u64) -> Option<&Block> {
        self.entries.iter().find(|e| e.id == id).map(|e| &e.block)
    }

    /// The command group with an id, for a change.
    /// @param id - the group's block id
    pub fn group_mut(&mut self, id: u64) -> Option<&mut CommandGroup> {
        match self.block_mut(id)? {
            Block::CommandGroup(group) => Some(group),
            _ => None,
        }
    }

    /// Removes every block.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.open_group = None;
        self.counted = 0;
    }

    /// How many blocks there are.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The blocks, oldest first.
    pub fn entries(&self) -> impl DoubleEndedIterator<Item = &Entry> {
        self.entries.iter()
    }

    /// The rows of the block at a position, laid out again only when needed.
    /// @param index - the position, oldest first
    /// @param width - the screen width
    /// @param ctx - the frame
    pub fn rows_at(&mut self, index: usize, width: usize, ctx: &LayoutCtx) -> Vec<Row> {
        let Some(entry) = self.entries.get_mut(index) else { return Vec::new() };
        if entry.block.is_live() {
            return entry.block.layout(entry.id, width, ctx);
        }
        match &entry.cache {
            Some((cached_width, rows)) if *cached_width == width => rows.clone(),
            _ => {
                let rows = entry.block.layout(entry.id, width, ctx);
                entry.cache = Some((width, rows.clone()));
                rows
            }
        }
    }

    /// The number of rows the block at a position takes.
    /// @param index - the position, oldest first
    /// @param width - the screen width
    /// @param ctx - the frame
    pub fn height_at(&mut self, index: usize, width: usize, ctx: &LayoutCtx) -> usize {
        if let Some(Entry { cache: Some((cached, rows)), .. }) = self.entries.get(index) {
            if *cached == width {
                return rows.len();
            }
        }
        self.rows_at(index, width, ctx).len()
    }

    /// The rows of every block together.
    /// @param width - the screen width
    /// @param ctx - the frame
    pub fn total_rows(&mut self, width: usize, ctx: &LayoutCtx) -> usize {
        (0..self.entries.len()).map(|i| self.height_at(i, width, ctx)).sum()
    }

    /// The position of the block with an id.
    /// @param id - the block id
    pub fn index_of(&self, id: u64) -> Option<usize> {
        self.entries.iter().position(|e| e.id == id)
    }

    /// Drops the oldest blocks while the transcript has more than `MAX_ROWS` rows at this width.
    /// Returns how many rows were dropped.
    /// @param width - the screen width
    /// @param ctx - the frame
    pub fn enforce_cap(&mut self, width: usize, ctx: &LayoutCtx) -> usize {
        let mut total = self.total_rows(width, ctx);
        let mut dropped = 0;
        while total > MAX_ROWS && self.entries.len() > 1 {
            let rows = self.height_at(0, width, ctx);
            let entry = self.entries.pop_front();
            if entry.is_some_and(|e| Some(e.id) == self.open_group) {
                self.open_group = None;
            }
            total -= rows;
            dropped += rows;
        }
        self.counted = total;
        dropped
    }

    /// True when any block draws something that changes with time, such as a running command.
    pub fn animating(&self) -> bool {
        self.entries.iter().any(|e| matches!(&e.block, Block::CommandGroup(g) if g.runs.iter().any(|r| matches!(r.state, super::block::RunState::Running { .. }))))
    }

    /// The newest command group, by id.
    #[cfg(test)]
    pub fn last_group(&self) -> Option<u64> {
        self.entries.iter().rev().find(|e| matches!(e.block, Block::CommandGroup(_))).map(|e| e.id)
    }
}

#[cfg(test)]
mod tests {
    use super::super::block::{RunKind, RunState};
    use super::*;

    fn ctx() -> LayoutCtx {
        LayoutCtx { color: false, now: std::time::Instant::now(), spinner: '⠋' }
    }

    #[test]
    fn runs_in_a_row_share_a_group_and_a_reply_closes_it() {
        let mut transcript = Transcript::default();
        let (a, _) = transcript.add_run(CommandRun::running(RunKind::Shell { command: "ls".into() }));
        let (b, index) = transcript.add_run(CommandRun::running(RunKind::Shell { command: "pwd".into() }));
        assert_eq!((a, index), (b, 1));
        transcript.push(Block::Reply { markdown: "done".into() });
        let (c, index) = transcript.add_run(CommandRun::running(RunKind::Shell { command: "ls".into() }));
        assert_ne!(a, c);
        assert_eq!(index, 0);
        assert!(transcript.animating());
        if let Some(group) = transcript.group_mut(c) {
            group.runs[0].state = RunState::Done;
        }
        if let Some(group) = transcript.group_mut(a) {
            group.runs.iter_mut().for_each(|r| r.state = RunState::Done);
        }
        assert!(!transcript.animating());
    }

    #[test]
    fn a_layout_is_cached_until_the_width_or_the_block_changes() {
        let mut transcript = Transcript::default();
        let id = transcript.push(Block::Text("hello world".into()));
        assert_eq!(transcript.rows_at(0, 10, &ctx()).len(), 2);
        assert_eq!(transcript.height_at(0, 10, &ctx()), 2);
        assert_eq!(transcript.height_at(0, 40, &ctx()), 1);
        if let Some(Block::Text(text)) = transcript.block_mut(id) {
            text.push_str(" again and again");
        }
        assert_eq!(transcript.height_at(0, 40, &ctx()), 1);
        assert_eq!(transcript.height_at(0, 10, &ctx()), 3);
    }

    #[test]
    fn the_oldest_blocks_go_when_the_cap_is_passed() {
        let mut transcript = Transcript::default();
        for n in 0..(MAX_ROWS + 10) {
            transcript.push(Block::Text(format!("line {n}")));
        }
        assert_eq!(transcript.enforce_cap(40, &ctx()), 10);
        assert_eq!(transcript.len(), MAX_ROWS);
    }
}

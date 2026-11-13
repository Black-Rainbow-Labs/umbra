//! The session journal (task-2617, E11): the session saved as JSON lines next to the trajectory,
//! appended after every step instead of rewriting the whole trajectory. Each new message is one
//! line, so a step writes about the size of what it added. The trajectory JSON, in the same
//! format as before, is written when a turn ends and when the run ends, and can be rebuilt from
//! the journal at any time.
//!
//! Lines, each a JSON object with a `t` field:
//! - `start`: the format, Umbra's version and the run's config. Always the first line.
//! - `msg`: one message appended to the history.
//! - `truncate`: the history was cut back to `len` messages; later `msg` lines follow from there.
//!   A compaction that rewrites earlier messages is a truncate followed by the rewritten messages.
//! - `info`: the parts of the trajectory's `info` block that changed since the last `info` line.
//!
//! A crash in the middle of a write leaves at most the last line cut off. The reader skips a
//! line that does not parse, so the session resumes from the line before it.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::models::message::Message;
use crate::models::trajectory::{Trajectory, TrajectoryInfo};

/// The value of the `format` field of the `start` line.
pub const JOURNAL_FORMAT: &str = "umbra-journal-1";

/// The journal that belongs to a trajectory file: `run.traj.json` has `run.traj.journal.jsonl`.
/// @param trajectory - the trajectory path
pub fn journal_path(trajectory: &Path) -> PathBuf {
    trajectory.with_extension("journal.jsonl")
}

/// True when a session's journal exists and is at least as new as its trajectory JSON, so the
/// journal is the session's record.
/// @param trajectory - the trajectory path
pub fn journal_is_newest(trajectory: &Path) -> bool {
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    match (modified(&journal_path(trajectory)), modified(trajectory)) {
        (Some(j), Some(t)) => j >= t,
        (Some(_), None) => true,
        _ => false,
    }
}

/// Appends a session to its journal, writing only what changed since the last write.
#[derive(Debug, Default)]
pub struct JournalWriter {
    path: PathBuf,
    /// A hash of each message already in the journal, in order.
    written: Vec<u64>,
    /// The last `info` parts written, as JSON text, by name.
    info: Vec<(String, String)>,
    started: bool,
}

impl JournalWriter {
    /// Builds a writer for a journal. The first write starts the file again, so a resumed or
    /// forked session gets one complete journal.
    /// @param path - the journal file
    pub fn new(path: PathBuf) -> Self {
        JournalWriter { path, ..Default::default() }
    }

    /// Writes what changed: new messages, a truncate when earlier messages changed, and the
    /// changed parts of `info`.
    /// @param info - the trajectory's info block now
    /// @param messages - the history now
    pub fn write(&mut self, info: &TrajectoryInfo, messages: &[Message]) -> Result<()> {
        let mut lines: Vec<String> = Vec::new();
        if !self.started {
            lines.push(json!({ "t": "start", "format": JOURNAL_FORMAT, "umbra_version": info.umbra_version, "config": info.config }).to_string());
        }
        let hashes: Vec<u64> = messages.iter().map(message_hash).collect();
        let same = self.written.iter().zip(&hashes).take_while(|(a, b)| a == b).count();
        if same < self.written.len() {
            lines.push(json!({ "t": "truncate", "len": same }).to_string());
        }
        for message in &messages[same..] {
            lines.push(json!({ "t": "msg", "m": message }).to_string());
        }
        if let Some(line) = self.changed_info(info) {
            lines.push(line);
        }
        self.append(&lines)?;
        self.written = hashes;
        Ok(())
    }

    /// Returns an `info` line holding the parts of `info` that changed, or None.
    /// @param info - the trajectory's info block now
    fn changed_info(&mut self, info: &TrajectoryInfo) -> Option<String> {
        let mut changed = serde_json::Map::new();
        for (name, value) in info_parts(info) {
            let text = value.to_string();
            match self.info.iter_mut().find(|(n, _)| n == name) {
                Some((_, last)) if *last == text => {}
                Some((_, last)) => {
                    *last = text;
                    changed.insert(name.into(), value);
                }
                None => {
                    self.info.push((name.into(), text));
                    changed.insert(name.into(), value);
                }
            }
        }
        if changed.is_empty() {
            return None;
        }
        changed.insert("t".into(), json!("info"));
        Some(Value::Object(changed).to_string())
    }

    /// Writes lines to the journal: the first write replaces the file, later ones append.
    /// @param lines - the lines, without line endings
    fn append(&mut self, lines: &[String]) -> Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let mut file = std::fs::OpenOptions::new().create(true).write(true).append(self.started).truncate(!self.started).open(&self.path).with_context(|| format!("opening {}", self.path.display()))?;
        let mut text = lines.join("\n");
        text.push('\n');
        file.write_all(text.as_bytes()).with_context(|| format!("writing {}", self.path.display()))?;
        self.started = true;
        Ok(())
    }
}

/// The parts of the info block an `info` line can hold, by name.
/// @param info - the trajectory's info block
fn info_parts(info: &TrajectoryInfo) -> [(&'static str, Value); 5] {
    [("model_stats", json!(info.model_stats)), ("exit_status", json!(info.exit_status)), ("submission", json!(info.submission)), ("compactions", json!(info.compactions)), ("session", json!(info.session))]
}

/// A writer that continues an existing journal: later writes append to it, the way they would had
/// the run never stopped, instead of writing the whole session again (task-2643, E18). A last line
/// cut off by a crash is removed first, so a new line is never joined to half of an old one.
/// Returns None, and the session is written whole as before, when the file is missing, is not an
/// Umbra journal, or has a line other than the last that does not read.
/// @param path - the journal file
pub fn continue_journal(path: &Path) -> Option<JournalWriter> {
    let text = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let (last, earlier) = lines.split_last()?;
    if earlier.iter().any(|line| !line.trim().is_empty() && serde_json::from_str::<Value>(line).is_err()) {
        return None;
    }
    let last_reads = serde_json::from_str::<Value>(last).is_ok();
    if !last_reads {
        let keep = text.len() - last.len();
        std::fs::OpenOptions::new().write(true).open(path).and_then(|file| file.set_len(keep as u64)).ok()?;
    } else if !last.ends_with('\n') {
        std::fs::OpenOptions::new().append(true).open(path).and_then(|mut file| file.write_all(b"\n")).ok()?;
    }
    let trajectory = load_journal(path).ok()?;
    let written = trajectory.messages.iter().map(message_hash).collect();
    let info = info_parts(&trajectory.info).into_iter().map(|(name, value)| (name.to_string(), value.to_string())).collect();
    Some(JournalWriter { path: path.to_path_buf(), written, info, started: true })
}

/// A hash of one message, to tell which messages are already in the journal.
/// @param message - the message
fn message_hash(message: &Message) -> u64 {
    let mut hasher = DefaultHasher::new();
    serde_json::to_string(message).unwrap_or_default().hash(&mut hasher);
    hasher.finish()
}

/// Rebuilds a trajectory from a journal. A last line that does not parse, as a crash in the
/// middle of a write leaves it, is skipped.
/// @param path - the journal file
pub fn load_journal(path: &Path) -> Result<Trajectory> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut info = json!({});
    let mut messages: Vec<Message> = Vec::new();
    let mut started = false;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(record) = serde_json::from_str::<Value>(line) else { continue };
        match record["t"].as_str() {
            Some("start") => {
                started = record["format"] == JOURNAL_FORMAT;
                info["config"] = record["config"].clone();
                info["umbra_version"] = record["umbra_version"].clone();
            }
            Some("msg") => messages.push(serde_json::from_value(record["m"].clone()).with_context(|| format!("a message in {} does not read", path.display()))?),
            Some("truncate") => messages.truncate(record["len"].as_u64().unwrap_or(0) as usize),
            Some("info") => record.as_object().into_iter().flatten().filter(|(k, _)| *k != "t").for_each(|(k, v)| info[k] = v.clone()),
            _ => {}
        }
    }
    if !started {
        bail!("{} is not an Umbra journal", path.display());
    }
    let info: TrajectoryInfo = serde_json::from_value(info).with_context(|| format!("the info in {} does not read", path.display()))?;
    Ok(Trajectory { info, messages, trajectory_format: "mini-swe-agent-1.1".into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::message::Role;

    /// A journal path in a fresh folder for one test.
    /// @param name - the test's name
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("umbra-journal-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("run.traj.journal.jsonl")
    }

    #[test]
    fn appends_new_messages_and_rebuilds_the_trajectory() {
        let path = scratch("append");
        let mut writer = JournalWriter::new(path.clone());
        let mut messages = vec![Message::system("s"), Message::user("u")];
        let mut info = TrajectoryInfo::default();
        writer.write(&info, &messages).unwrap();
        let first = std::fs::metadata(&path).unwrap().len();
        messages.push(Message::new(Role::Assistant, "a"));
        info.model_stats.api_calls = 1;
        writer.write(&info, &messages).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().filter(|l| l.contains("\"t\":\"msg\"")).count(), 3, "each message is written once");
        assert!(std::fs::metadata(&path).unwrap().len() - first < 400, "a step appends only what it added");
        let loaded = load_journal(&path).unwrap();
        assert_eq!(loaded.messages.len(), 3);
        assert_eq!(loaded.info.model_stats.api_calls, 1);
    }

    #[test]
    fn a_rewritten_message_is_a_truncate_and_the_rest_again() {
        let path = scratch("rewrite");
        let mut writer = JournalWriter::new(path.clone());
        let mut messages = vec![Message::system("s"), Message::user("u"), Message::tool("1", "long output"), Message::new(Role::Assistant, "a")];
        writer.write(&TrajectoryInfo::default(), &messages).unwrap();
        messages[2].content = "[masked]".into();
        writer.write(&TrajectoryInfo::default(), &messages).unwrap();
        let loaded = load_journal(&path).unwrap();
        assert_eq!(loaded.messages.len(), 4);
        assert_eq!(loaded.messages[2].content, "[masked]");
    }

    #[test]
    fn a_continued_journal_appends_instead_of_starting_again() {
        let path = scratch("continue");
        let mut writer = JournalWriter::new(path.clone());
        let mut messages = vec![Message::system("s"), Message::user("u"), Message::new(Role::Assistant, "a"), Message::exit("Submitted", "done")];
        writer.write(&TrajectoryInfo::default(), &messages).unwrap();
        let before = std::fs::read_to_string(&path).unwrap();
        messages.pop();
        messages.push(Message::user("next"));
        let mut continued = continue_journal(&path).expect("an Umbra journal continues");
        continued.write(&TrajectoryInfo::default(), &messages).unwrap();
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.starts_with(&before), "the old lines are kept as they were");
        assert_eq!(after[before.len()..].lines().count(), 2, "a truncate past the exit marker, then the new message");
        let loaded = load_journal(&path).unwrap();
        assert_eq!(loaded.messages.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(), ["s", "u", "a", "next"]);
    }

    #[test]
    fn a_continued_journal_drops_a_cut_last_line_before_appending() {
        let path = scratch("continue-cut");
        let mut writer = JournalWriter::new(path.clone());
        let mut messages = vec![Message::system("s"), Message::user("remember PINE-7")];
        writer.write(&TrajectoryInfo::default(), &messages).unwrap();
        messages.push(Message::new(Role::Assistant, "a long reply that will be cut off part way through"));
        writer.write(&TrajectoryInfo::default(), &messages).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, &bytes[..bytes.len() - 30]).unwrap();
        let mut continued = continue_journal(&path).expect("a cut journal continues");
        messages.truncate(2);
        messages.push(Message::user("carry on"));
        continued.write(&TrajectoryInfo::default(), &messages).unwrap();
        let loaded = load_journal(&path).unwrap();
        assert_eq!(loaded.messages.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(), ["s", "remember PINE-7", "carry on"]);
        std::fs::write(&path, "{\"t\":\"start\",\"format\":\"umbra-journal-1\"}\nnot json\n{\"t\":\"msg\"}\n").unwrap();
        assert!(continue_journal(&path).is_none(), "a bad line before the last is not appended to");
    }

    #[test]
    fn a_journal_cut_in_the_middle_of_the_last_line_still_loads() {
        let path = scratch("cut");
        let mut writer = JournalWriter::new(path.clone());
        let mut messages = vec![Message::system("s"), Message::user("remember PINE-7")];
        writer.write(&TrajectoryInfo::default(), &messages).unwrap();
        messages.push(Message::new(Role::Assistant, "a long reply that will be cut off part way through"));
        writer.write(&TrajectoryInfo::default(), &messages).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, &bytes[..bytes.len() - 30]).unwrap();
        let loaded = load_journal(&path).unwrap();
        assert_eq!(loaded.messages.len(), 2, "the cut line is skipped");
        assert_eq!(loaded.messages[1].content, "remember PINE-7");
    }
}

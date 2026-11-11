//! Drives the fullscreen view from a test. `UMBRA_FORCE_FULLSCREEN=1` makes Umbra open the view
//! over pipes; the test writes keys, text and SGR mouse reports to stdin and reads the frames from
//! stdout. Every frame is a full repaint between `ESC[?2026h` and `ESC[?2026l`, so the last
//! complete frame is the screen, and no terminal emulation is needed.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The style of one cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub bold: bool,
    pub reverse: bool,
}

/// One frame: its rows as plain text, and the cells with their styles.
#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub rows: Vec<String>,
    pub cells: Vec<Vec<Cell>>,
}

impl Frame {
    /// The whole frame as text, one row per line.
    pub fn text(&self) -> String {
        self.rows.join("\n")
    }

    /// The first row that contains the text.
    /// @param needle - the text
    pub fn row_of(&self, needle: &str) -> Option<usize> {
        self.rows.iter().position(|r| r.contains(needle))
    }

    /// The column of the text in its first row, counted in characters.
    /// @param needle - the text
    pub fn position_of(&self, needle: &str) -> Option<(usize, usize)> {
        let row = self.row_of(needle)?;
        let at = self.rows[row].find(needle)?;
        Some((self.rows[row][..at].chars().count(), row))
    }

    /// The text of the cells drawn in reverse video, row by row.
    pub fn reversed(&self) -> Vec<String> {
        self.cells.iter().map(|row| row.iter().filter(|c| c.reverse).map(|c| c.ch).collect::<String>()).filter(|s| !s.is_empty()).collect()
    }

    /// The input line: the row above the status bar, which is the last row (task-2625). Without a
    /// status bar the input line is the last row.
    pub fn input(&self) -> &str {
        let last = self.rows.last().map(String::as_str).unwrap_or("");
        if self.rows.len() >= 2 && !last.starts_with('❯') && !last.starts_with('›') {
            return &self.rows[self.rows.len() - 2];
        }
        last
    }

    /// The status bar: the last row.
    pub fn status(&self) -> &str {
        self.rows.last().map(String::as_str).unwrap_or("")
    }
}

/// Reads one frame's body: rows split at `\r\n`, escape sequences applied to the cell styles.
/// @param body - the text between `ESC[H` and `ESC[J`
fn parse_rows(body: &str) -> Frame {
    let mut frame = Frame::default();
    for raw in body.split("\r\n") {
        let (mut text, mut cells) = (String::new(), Vec::new());
        let (mut bold, mut reverse) = (false, false);
        let mut chars = raw.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '\x1b' {
                if (c as u32) >= 0x20 {
                    text.push(c);
                    cells.push(Cell { ch: c, bold, reverse });
                }
                continue;
            }
            match chars.next() {
                Some('[') => {
                    let mut params = String::new();
                    let mut last = ' ';
                    for d in chars.by_ref() {
                        if ('@'..='~').contains(&d) {
                            last = d;
                            break;
                        }
                        params.push(d);
                    }
                    if last == 'm' {
                        apply_sgr(&params, &mut bold, &mut reverse);
                    }
                }
                Some(']') => {
                    while let Some(d) = chars.next() {
                        if d == '\x07' || (d == '\x1b' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        frame.rows.push(text);
        frame.cells.push(cells);
    }
    frame
}

/// Applies SGR parameters to the bold and reverse flags.
/// @param params - the parameters, such as `0` or `1;33`
/// @param bold - the bold flag
/// @param reverse - the reverse flag
fn apply_sgr(params: &str, bold: &mut bool, reverse: &mut bool) {
    for p in params.split(';') {
        match p {
            "" | "0" => {
                *bold = false;
                *reverse = false;
            }
            "1" => *bold = true,
            "22" => *bold = false,
            "7" => *reverse = true,
            "27" => *reverse = false,
            _ => {}
        }
    }
}

/// The last complete frame in the bytes written so far.
/// @param output - stdout so far
pub fn last_frame(output: &str) -> Option<Frame> {
    let end = output.rfind("\x1b[?2026l")?;
    let start = output[..end].rfind("\x1b[?2026h")?;
    let frame = &output[start..end];
    let body = frame.split_once("\x1b[H")?.1;
    let body = body.rsplit_once("\x1b[J").map(|(b, _)| b).unwrap_or(body);
    Some(parse_rows(body))
}

/// A running umbra in the fullscreen view.
pub struct Screen {
    child: Child,
    stdin: Option<ChildStdin>,
    output: Arc<Mutex<Vec<u8>>>,
}

impl Screen {
    /// Starts umbra in the fullscreen view with a fixed size.
    /// @param home - the fake home folder
    /// @param args - the command line
    /// @param envs - extra environment variables
    /// @param size - the columns and rows
    pub fn start(home: &Path, args: &[&str], envs: &[(&str, &str)], size: (usize, usize)) -> Self {
        let mut command = super::umbra_command(home, args);
        let (cols, rows) = (size.0.to_string(), size.1.to_string());
        command.env("UMBRA_FORCE_FULLSCREEN", "1").env("COLUMNS", &cols).env("LINES", &rows).env("UMBRA_BANNER_ANIMATION", "0").env("UMBRA_STATUS_BAR", "1");
        command.env_remove("ANTHROPIC_API_KEY").env_remove("ANTHROPIC_AUTH_TOKEN").env_remove("OPENAI_API_KEY");
        command.envs(envs.iter().copied()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn().expect("umbra binary runs");
        let stdin = child.stdin.take();
        let mut stdout = child.stdout.take().unwrap();
        let output = Arc::new(Mutex::new(Vec::new()));
        let sink = output.clone();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 65536];
            while let Ok(n) = stdout.read(&mut buffer) {
                if n == 0 {
                    break;
                }
                sink.lock().unwrap().extend_from_slice(&buffer[..n]);
            }
        });
        Screen { child, stdin, output }
    }

    /// Everything umbra wrote to stdout so far.
    pub fn output(&self) -> String {
        String::from_utf8_lossy(&self.output.lock().unwrap()).into_owned()
    }

    /// The last complete frame.
    pub fn frame(&self) -> Frame {
        last_frame(&self.output()).unwrap_or_default()
    }

    /// Waits until a frame satisfies a test, and returns it. Panics with the last frame after the
    /// time is up.
    /// @param what - what is waited for, for the message
    /// @param limit - the longest wait
    /// @param test - the test
    pub fn wait_until(&self, what: &str, limit: Duration, test: impl Fn(&Frame) -> bool) -> Frame {
        let started = Instant::now();
        loop {
            let frame = self.frame();
            if test(&frame) {
                return frame;
            }
            if started.elapsed() > limit {
                panic!("timed out waiting for {what}. Last frame:\n{}", frame.text());
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    /// Waits until a row contains the text.
    /// @param text - the text
    pub fn wait_for(&self, text: &str) -> Frame {
        self.wait_until(text, Duration::from_secs(15), |f| f.row_of(text).is_some())
    }

    /// Writes bytes to umbra's stdin.
    /// @param bytes - the bytes
    pub fn send(&mut self, bytes: &[u8]) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        stdin.write_all(bytes).unwrap();
        stdin.flush().unwrap();
    }

    /// Types text, without Enter.
    /// @param text - the text
    pub fn type_text(&mut self, text: &str) {
        self.send(text.as_bytes());
    }

    /// Types a line and presses Enter.
    /// @param text - the text
    pub fn type_line(&mut self, text: &str) {
        self.send(format!("{text}\r").as_bytes());
    }

    /// Presses a named key: Enter, Tab, BackTab, Esc, Up, Down, Right, Left, Space, Backspace, PageUp.
    /// @param name - the key
    pub fn press(&mut self, name: &str) {
        let bytes: &[u8] = match name {
            "Enter" => b"\r",
            "Tab" => b"\t",
            "BackTab" => b"\x1b[Z",
            "Esc" => b"\x1b",
            "Up" => b"\x1b[A",
            "Down" => b"\x1b[B",
            "Right" => b"\x1b[C",
            "Left" => b"\x1b[D",
            "Space" => b" ",
            "Backspace" => b"\x7f",
            "PageUp" => b"\x1b[5~",
            other => panic!("unknown key {other}"),
        };
        self.send(bytes);
    }

    /// Clicks a cell, counted from 0: a press and a release on the same cell.
    /// @param col - the column
    /// @param row - the row
    pub fn click(&mut self, col: usize, row: usize) {
        self.send(format!("\x1b[<0;{};{}M\x1b[<0;{};{}m", col + 1, row + 1, col + 1, row + 1).as_bytes());
    }

    /// Clicks the first character of a text on screen.
    /// @param text - the text
    pub fn click_text(&mut self, text: &str) {
        let frame = self.wait_for(text);
        let (col, row) = frame.position_of(text).unwrap();
        self.click(col, row);
    }

    /// Drags from one cell to another with the left button.
    /// @param from - the start, as (column, row)
    /// @param to - the end, as (column, row)
    pub fn drag(&mut self, from: (usize, usize), to: (usize, usize)) {
        let mut bytes = format!("\x1b[<0;{};{}M", from.0 + 1, from.1 + 1);
        bytes.push_str(&format!("\x1b[<32;{};{}M", to.0 + 1, to.1 + 1));
        bytes.push_str(&format!("\x1b[<0;{};{}m", to.0 + 1, to.1 + 1));
        self.send(bytes.as_bytes());
    }

    /// Closes stdin and waits for umbra to exit. Returns stdout and stderr together and the exit
    /// code. Kills it and panics when it does not exit in time.
    /// @param limit - the longest wait
    pub fn finish(mut self, limit: Duration) -> (String, Option<i32>) {
        self.stdin.take();
        let started = Instant::now();
        while started.elapsed() < limit {
            if let Some(status) = self.child.try_wait().unwrap() {
                std::thread::sleep(Duration::from_millis(100));
                let mut stderr = String::new();
                if let Some(mut pipe) = self.child.stderr.take() {
                    let _ = pipe.read_to_string(&mut stderr);
                }
                return (format!("{}{stderr}", self.output()), status.code());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        panic!("umbra did not exit within {limit:?}. Last frame:\n{}", self.frame().text());
    }
}

impl Drop for Screen {
    /// Kills umbra when a test ends without finishing it.
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

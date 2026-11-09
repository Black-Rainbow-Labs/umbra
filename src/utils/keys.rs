//! Turns the bytes a terminal sends in raw input mode into keys: text, editing keys, the keys
//! that scroll, mouse wheel reports and bracketed paste. Used by the fullscreen view, which reads
//! input a key at a time instead of a line at a time.

/// One key, or one paste, read from the terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// Printable text, one or more characters.
    Text(String),
    /// Text pasted in one piece, line breaks kept.
    Paste(String),
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    /// Ctrl+Home: the top of the transcript.
    Top,
    /// Ctrl+End: the bottom of the transcript.
    Bottom,
    WheelUp,
    WheelDown,
    Escape,
    /// Ctrl+D, the end of input on an empty line.
    CtrlD,
    /// Ctrl+U, clear the input line.
    CtrlU,
}

/// The bytes that start and end a bracketed paste.
const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";

/// Reads keys out of the byte stream. Bytes that end in the middle of a key, an escape sequence or
/// a UTF-8 character are kept until the next call.
#[derive(Default)]
pub struct KeyDecoder {
    pending: Vec<u8>,
    /// The text of a paste in progress, when a paste start was read and its end was not yet.
    paste: Option<Vec<u8>>,
}

impl KeyDecoder {
    /// Adds bytes from the terminal and returns the keys they complete.
    /// @param bytes - what the terminal sent
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Key> {
        self.pending.extend_from_slice(bytes);
        let mut keys = Vec::new();
        loop {
            if self.paste.is_some() {
                if !self.take_paste(&mut keys) {
                    break;
                }
                continue;
            }
            match decode_one(&self.pending) {
                Decoded::Key(key, used) => {
                    self.pending.drain(..used);
                    push_key(&mut keys, key);
                }
                Decoded::Skip(used) => {
                    self.pending.drain(..used);
                }
                Decoded::PasteStart(used) => {
                    self.pending.drain(..used);
                    self.paste = Some(Vec::new());
                }
                Decoded::Incomplete => break,
            }
        }
        keys
    }

    /// Moves pasted bytes out of `pending` until the paste end. Returns true when the paste ended
    /// and the pasted text was added as one key.
    /// @param keys - the keys read so far
    fn take_paste(&mut self, keys: &mut Vec<Key>) -> bool {
        let paste = self.paste.as_mut().expect("a paste is in progress");
        match find(&self.pending, PASTE_END) {
            Some(at) => {
                paste.extend_from_slice(&self.pending[..at]);
                self.pending.drain(..at + PASTE_END.len());
                let text = String::from_utf8_lossy(paste).replace("\r\n", "\n").replace('\r', "\n");
                self.paste = None;
                keys.push(Key::Paste(text));
                true
            }
            None => {
                // Keep a tail that could be the start of the paste end.
                let keep = PASTE_END.len().saturating_sub(1).min(self.pending.len());
                let cut = self.pending.len() - keep;
                paste.extend_from_slice(&self.pending[..cut]);
                self.pending.drain(..cut);
                false
            }
        }
    }
}

/// Adds a key, joining text that follows text so a burst of typing is one key.
/// @param keys - the keys read so far
/// @param key - the next key
fn push_key(keys: &mut Vec<Key>, key: Key) {
    if let (Some(Key::Text(last)), Key::Text(next)) = (keys.last_mut(), &key) {
        last.push_str(next);
        return;
    }
    keys.push(key);
}

/// What the start of the byte stream holds.
enum Decoded {
    Key(Key, usize),
    /// Bytes to drop, such as an unknown escape sequence or a mouse click.
    Skip(usize),
    PasteStart(usize),
    Incomplete,
}

/// Reads the first key from the bytes.
/// @param bytes - the bytes not yet read
fn decode_one(bytes: &[u8]) -> Decoded {
    let Some(&first) = bytes.first() else { return Decoded::Incomplete };
    match first {
        0x1b => decode_escape(bytes),
        b'\r' | b'\n' => Decoded::Key(Key::Enter, 1),
        0x7f | 0x08 => Decoded::Key(Key::Backspace, 1),
        0x04 => Decoded::Key(Key::CtrlD, 1),
        0x15 => Decoded::Key(Key::CtrlU, 1),
        b'\t' => Decoded::Key(Key::Text("    ".into()), 1),
        0x00..=0x1f => Decoded::Skip(1),
        _ => decode_utf8(bytes),
    }
}

/// Reads one UTF-8 character.
/// @param bytes - the bytes, starting with a character's first byte
fn decode_utf8(bytes: &[u8]) -> Decoded {
    let length = match bytes[0] {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => return Decoded::Skip(1),
    };
    if bytes.len() < length {
        return Decoded::Incomplete;
    }
    match std::str::from_utf8(&bytes[..length]) {
        Ok(text) => Decoded::Key(Key::Text(text.to_string()), length),
        Err(_) => Decoded::Skip(1),
    }
}

/// Reads an escape sequence: CSI (`ESC [`), SS3 (`ESC O`), or Alt plus a key. A lone ESC at the
/// end of the bytes waits for the next byte, because it may start a sequence.
/// @param bytes - the bytes, starting with ESC
fn decode_escape(bytes: &[u8]) -> Decoded {
    let Some(&second) = bytes.get(1) else { return Decoded::Incomplete };
    match second {
        b'[' => decode_csi(bytes),
        b'O' => match bytes.get(2) {
            None => Decoded::Incomplete,
            Some(&final_byte) => match ss3_key(final_byte) {
                Some(key) => Decoded::Key(key, 3),
                None => Decoded::Skip(3),
            },
        },
        0x1b => Decoded::Key(Key::Escape, 1),
        _ => Decoded::Key(Key::Escape, 1),
    }
}

/// The key an SS3 sequence (`ESC O x`) stands for.
/// @param final_byte - the byte after `ESC O`
fn ss3_key(final_byte: u8) -> Option<Key> {
    Some(match final_byte {
        b'A' => Key::Up,
        b'B' => Key::Down,
        b'C' => Key::Right,
        b'D' => Key::Left,
        b'H' => Key::Home,
        b'F' => Key::End,
        _ => return None,
    })
}

/// Reads a CSI sequence: `ESC [`, parameter bytes, then a final byte from `@` to `~`.
/// @param bytes - the bytes, starting with `ESC [`
fn decode_csi(bytes: &[u8]) -> Decoded {
    let Some(end) = bytes.iter().skip(2).position(|b| (0x40..=0x7e).contains(b)).map(|i| i + 2) else {
        return if bytes.len() > 64 { Decoded::Skip(bytes.len()) } else { Decoded::Incomplete };
    };
    let used = end + 1;
    if bytes[..used] == *PASTE_START {
        return Decoded::PasteStart(used);
    }
    let params = std::str::from_utf8(&bytes[2..end]).unwrap_or("");
    match csi_key(params, bytes[end]) {
        Some(key) => Decoded::Key(key, used),
        None => Decoded::Skip(used),
    }
}

/// The key a CSI sequence stands for, or None for one Umbra does not use.
/// @param params - the parameter text between `ESC [` and the final byte
/// @param final_byte - the final byte
fn csi_key(params: &str, final_byte: u8) -> Option<Key> {
    if let Some(mouse) = params.strip_prefix('<') {
        return mouse_key(mouse, final_byte);
    }
    let mut parts = params.split(';');
    let first = parts.next().unwrap_or("");
    let ctrl = parts.next().is_some_and(|m| m == "5");
    Some(match (final_byte, first) {
        (b'A', _) => Key::Up,
        (b'B', _) => Key::Down,
        (b'C', _) => Key::Right,
        (b'D', _) => Key::Left,
        (b'H', _) if ctrl => Key::Top,
        (b'F', _) if ctrl => Key::Bottom,
        (b'H', _) => Key::Home,
        (b'F', _) => Key::End,
        (b'~', "1" | "7") if ctrl => Key::Top,
        (b'~', "4" | "8") if ctrl => Key::Bottom,
        (b'~', "1" | "7") => Key::Home,
        (b'~', "4" | "8") => Key::End,
        (b'~', "3") => Key::Delete,
        (b'~', "5") => Key::PageUp,
        (b'~', "6") => Key::PageDown,
        _ => return None,
    })
}

/// The wheel key in an SGR mouse report (`ESC [ < button ; x ; y M`). Clicks and moves are
/// dropped.
/// @param params - the report after the `<`
/// @param final_byte - `M` for a press, `m` for a release
fn mouse_key(params: &str, final_byte: u8) -> Option<Key> {
    if final_byte != b'M' {
        return None;
    }
    match params.split(';').next()?.parse::<u32>().ok()? & !(4 | 8 | 16) {
        64 => Some(Key::WheelUp),
        65 => Some(Key::WheelDown),
        _ => None,
    }
}

/// The position of `needle` in `haystack`.
/// @param haystack - the bytes to search
/// @param needle - the bytes to find
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_text_editing_keys_and_enter() {
        let mut decoder = KeyDecoder::default();
        assert_eq!(decoder.feed(b"hi\x7f!\r"), vec![Key::Text("hi".into()), Key::Backspace, Key::Text("!".into()), Key::Enter]);
        assert_eq!(decoder.feed("é→".as_bytes()), vec![Key::Text("é→".into())]);
        assert_eq!(decoder.feed(b"\x04\x15"), vec![Key::CtrlD, Key::CtrlU]);
    }

    #[test]
    fn waits_for_a_sequence_or_character_split_across_reads() {
        let mut decoder = KeyDecoder::default();
        assert!(decoder.feed(b"\x1b[5").is_empty());
        assert_eq!(decoder.feed(b"~"), vec![Key::PageUp]);
        let bytes = "é".as_bytes();
        assert!(decoder.feed(&bytes[..1]).is_empty());
        assert_eq!(decoder.feed(&bytes[1..]), vec![Key::Text("é".into())]);
        assert!(decoder.feed(b"\x1b").is_empty());
        assert_eq!(decoder.feed(b"x"), vec![Key::Escape, Key::Text("x".into())]);
    }

    #[test]
    fn reads_the_scroll_keys_and_the_wheel() {
        let mut decoder = KeyDecoder::default();
        let keys = decoder.feed(b"\x1b[6~\x1b[1;5F\x1b[1;5H\x1b[F\x1bOH\x1b[<64;10;5M\x1b[<65;10;5M\x1b[<0;3;3M\x1b[<0;3;3m\x1b[3~");
        assert_eq!(keys, vec![Key::PageDown, Key::Bottom, Key::Top, Key::End, Key::Home, Key::WheelUp, Key::WheelDown, Key::Delete]);
    }

    #[test]
    fn reads_a_bracketed_paste_as_one_key() {
        let mut decoder = KeyDecoder::default();
        assert!(decoder.feed(b"\x1b[200~line one\r\nline").is_empty());
        assert_eq!(decoder.feed(b" two\x1b[201~x"), vec![Key::Paste("line one\nline two".into()), Key::Text("x".into())]);
    }
}

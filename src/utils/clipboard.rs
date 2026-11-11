//! Copying a selection to the clipboard. Umbra writes an OSC 52 sequence, which many terminals
//! turn into a clipboard write, and also writes the system clipboard itself, because Terminal.app
//! does not support OSC 52 and iTerm2 blocks it by default. Umbra never reads the clipboard.

use std::io::Write;

use base64::Engine;

/// Copies text: OSC 52 to the terminal, then the system clipboard. `UMBRA_CLIPBOARD_FILE`, for
/// tests, writes the text to that file in place of the system clipboard.
/// @param text - the text
pub fn copy(text: &str) -> Result<(), String> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(format!("\x1b]52;c;{encoded}\x07").as_bytes());
    let _ = stdout.flush();
    drop(stdout);
    if let Some(path) = std::env::var_os("UMBRA_CLIPBOARD_FILE") {
        return std::fs::write(path, text).map_err(|e| e.to_string());
    }
    native_copy(text)
}

/// The key that makes the terminal select text itself while Umbra has the mouse.
pub fn terminal_modifier() -> &'static str {
    if cfg!(target_os = "macos") {
        if std::env::var("TERM_PROGRAM").is_ok_and(|t| t == "iTerm.app") { "Option" } else { "Fn" }
    } else {
        "Shift"
    }
}

/// Writes the Windows clipboard as Unicode text.
/// @param text - the text
#[cfg(windows)]
fn native_copy(text: &str) -> Result<(), String> {
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    const CF_UNICODETEXT: u32 = 13;
    let wide: Vec<u16> = text.replace('\n', "\r\n").encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: plain Win32 clipboard calls. The memory is handed to the clipboard on success and
    // freed by us on failure; the clipboard is always closed.
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err("the clipboard is in use".into());
        }
        EmptyClipboard();
        let memory = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
        if memory.is_null() {
            CloseClipboard();
            return Err("out of memory".into());
        }
        let target = GlobalLock(memory) as *mut u16;
        std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
        GlobalUnlock(memory);
        let placed = !SetClipboardData(CF_UNICODETEXT, memory as _).is_null();
        if !placed {
            GlobalFree(memory);
        }
        CloseClipboard();
        if placed { Ok(()) } else { Err("the clipboard refused the text".into()) }
    }
}

/// Writes the macOS or Linux clipboard with the first program that exists: `pbcopy`, then
/// `wl-copy`, `xclip` and `xsel`.
/// @param text - the text
#[cfg(not(windows))]
fn native_copy(text: &str) -> Result<(), String> {
    let programs: &[(&str, &[&str])] = if cfg!(target_os = "macos") { &[("pbcopy", &[])] } else { &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"]), ("xsel", &["--clipboard", "--input"])] };
    for (program, args) in programs {
        let Ok(mut child) = std::process::Command::new(program).args(*args).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn() else { continue };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        return match child.wait() {
            Ok(status) if status.success() => Ok(()),
            _ => Err(format!("{program} failed")),
        };
    }
    // OSC 52 was written; with no clipboard program that is all that can be done.
    Ok(())
}

//! Raw input mode for the fullscreen view: keys arrive one at a time without echo, while Ctrl+C
//! still reaches Umbra as a signal (`ISIG` on Unix, `ENABLE_PROCESSED_INPUT` on Windows), so the
//! Ctrl+C handling in `interrupt` works the same in both views. Also the terminal's height.

use std::sync::Mutex;

/// The input mode saved when raw mode was turned on, restored when it is turned off.
static SAVED: Mutex<Option<SavedMode>> = Mutex::new(None);

#[cfg(unix)]
type SavedMode = libc::termios;

#[cfg(windows)]
type SavedMode = u32;

#[cfg(not(any(unix, windows)))]
type SavedMode = ();

/// Turns raw input mode on and remembers the mode it replaced. Returns false when stdin is not a
/// terminal or the mode cannot be changed.
pub fn enable_raw() -> bool {
    let mut saved = SAVED.lock().unwrap_or_else(|e| e.into_inner());
    if saved.is_some() {
        return true;
    }
    match set_raw() {
        Some(previous) => {
            *saved = Some(previous);
            true
        }
        None => false,
    }
}

/// Puts back the input mode raw mode replaced. Does nothing when raw mode is off. Uses `try_lock`
/// so it can run from a panic hook or a signal path without waiting on a lock.
pub fn restore() {
    let Ok(mut saved) = SAVED.try_lock() else { return };
    if let Some(previous) = saved.take() {
        put_back(previous);
    }
}

/// Turns raw mode on again when something, such as a program Umbra ran, changed the input mode.
/// Only Windows needs this: a console program can leave the console's mode changed when it exits.
pub fn reassert_raw() {
    #[cfg(windows)]
    {
        let saved = SAVED.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(previous) = *saved {
            let wanted = raw_console_mode(previous);
            if current_console_mode().is_some_and(|mode| mode != wanted) {
                set_console_mode(wanted);
            }
        }
    }
}

/// Turns off line buffering and echo on the terminal on stdin, keeping signals.
#[cfg(unix)]
fn set_raw() -> Option<SavedMode> {
    // SAFETY: tcgetattr and tcsetattr read and write a termios struct for the terminal on stdin.
    unsafe {
        let mut previous: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(libc::STDIN_FILENO, &mut previous) != 0 {
            return None;
        }
        let mut raw = previous;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO | libc::IEXTEN);
        raw.c_iflag &= !(libc::IXON | libc::ICRNL);
        raw.c_cc[libc::VMIN] = 1;
        raw.c_cc[libc::VTIME] = 0;
        (libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw) == 0).then_some(previous)
    }
}

/// Puts the saved terminal mode back.
/// @param previous - the mode saved by `set_raw`
#[cfg(unix)]
fn put_back(previous: SavedMode) {
    // SAFETY: writes back a termios struct read from the same terminal.
    unsafe {
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &previous);
    }
}

/// The console input mode for the fullscreen view: no line input, no echo, no quick edit (which
/// would take the mouse), virtual terminal input on, Ctrl+C still processed by the system.
/// @param previous - the mode before
#[cfg(windows)]
fn raw_console_mode(previous: u32) -> u32 {
    use windows_sys::Win32::System::Console::{ENABLE_ECHO_INPUT, ENABLE_EXTENDED_FLAGS, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT, ENABLE_QUICK_EDIT_MODE, ENABLE_VIRTUAL_TERMINAL_INPUT};
    (previous & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_QUICK_EDIT_MODE)) | ENABLE_PROCESSED_INPUT | ENABLE_VIRTUAL_TERMINAL_INPUT | ENABLE_EXTENDED_FLAGS
}

/// The console's current input mode, or None when stdin is not a console.
#[cfg(windows)]
fn current_console_mode() -> Option<u32> {
    use windows_sys::Win32::System::Console::{GetConsoleMode, GetStdHandle, STD_INPUT_HANDLE};
    // SAFETY: the handle is the process's own stdin and the mode is written by the call.
    unsafe {
        let mut mode = 0;
        (GetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), &mut mode) != 0).then_some(mode)
    }
}

/// Sets the console's input mode. Returns true when it worked.
/// @param mode - the new mode
#[cfg(windows)]
fn set_console_mode(mode: u32) -> bool {
    use windows_sys::Win32::System::Console::{GetStdHandle, SetConsoleMode, STD_INPUT_HANDLE};
    // SAFETY: the handle is the process's own stdin.
    unsafe { SetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), mode) != 0 }
}

/// Switches the console to the fullscreen input mode.
#[cfg(windows)]
fn set_raw() -> Option<SavedMode> {
    let previous = current_console_mode()?;
    set_console_mode(raw_console_mode(previous)).then_some(previous)
}

/// Puts the saved console mode back.
/// @param previous - the mode saved by `set_raw`
#[cfg(windows)]
fn put_back(previous: SavedMode) {
    set_console_mode(previous);
}

/// No raw mode on other systems.
#[cfg(not(any(unix, windows)))]
fn set_raw() -> Option<SavedMode> {
    None
}

/// Nothing to put back on other systems.
#[cfg(not(any(unix, windows)))]
fn put_back(_previous: SavedMode) {}

/// The terminal's height in rows: asked of the terminal, then `LINES`, then 24.
pub fn terminal_height() -> usize {
    console_height().or_else(|| std::env::var("LINES").ok().and_then(|l| l.trim().parse().ok())).filter(|h| *h > 0).unwrap_or(24)
}

/// Asks the Windows console for the height of its visible window.
#[cfg(windows)]
fn console_height() -> Option<usize> {
    use windows_sys::Win32::System::Console::{GetConsoleScreenBufferInfo, GetStdHandle, CONSOLE_SCREEN_BUFFER_INFO, STD_OUTPUT_HANDLE};
    // SAFETY: the handle is the process's own stdout and the struct is written by the call.
    unsafe {
        let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
        if GetConsoleScreenBufferInfo(GetStdHandle(STD_OUTPUT_HANDLE), &mut info) == 0 {
            return None;
        }
        Some((info.srWindow.Bottom - info.srWindow.Top + 1).max(0) as usize)
    }
}

/// Asks the terminal for its height.
#[cfg(unix)]
fn console_height() -> Option<usize> {
    // SAFETY: TIOCGWINSZ writes a winsize struct for the terminal on stdout.
    unsafe {
        let mut size: libc::winsize = std::mem::zeroed();
        (libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) == 0 && size.ws_row > 0).then_some(size.ws_row as usize)
    }
}

/// No way to ask on other systems.
#[cfg(not(any(windows, unix)))]
fn console_height() -> Option<usize> {
    None
}

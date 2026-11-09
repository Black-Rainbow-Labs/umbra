//! Ctrl+C. The handler only sets a flag; the agent loop, the environment, the providers and the
//! console check it and stop what they are doing. A third press that Umbra has not handled exits
//! the process, for the case where the main thread is stuck somewhere that never checks.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Once;
use std::time::{Duration, Instant};

/// Set by Ctrl+C, cleared when Umbra handles the interrupt.
static PENDING: AtomicBool = AtomicBool::new(false);

/// Presses since Umbra last handled one.
static PRESSES: AtomicU32 = AtomicU32::new(0);

static INSTALL: Once = Once::new();

/// The presses that force an exit when Umbra has not handled the earlier ones.
const FORCE_EXIT_PRESSES: u32 = 3;

/// How often a waiting loop looks at the flag.
pub const POLL: Duration = Duration::from_millis(100);

/// Installs the Ctrl+C handler once. A failure (another handler already set) is printed and
/// otherwise ignored, so Umbra still runs with the default Ctrl+C behavior.
pub fn install() {
    INSTALL.call_once(|| {
        enable_ctrl_c();
        if let Err(error) = ctrlc::set_handler(on_press) {
            eprintln!("umbra: could not install the Ctrl+C handler: {error}");
        }
    });
}

/// A Windows process started in a new process group, which some launchers do, has Ctrl+C turned
/// off and would never see the key. This turns it back on.
#[cfg(windows)]
fn enable_ctrl_c() {
    // SAFETY: a null handler with FALSE only clears the process's "ignore Ctrl+C" flag.
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 0);
    }
}

/// Unix has no such flag.
#[cfg(not(windows))]
fn enable_ctrl_c() {}

/// Records one press, and exits on the third unhandled one.
fn on_press() {
    PENDING.store(true, Ordering::SeqCst);
    let presses = PRESSES.fetch_add(1, Ordering::SeqCst) + 1;
    if presses >= FORCE_EXIT_PRESSES {
        eprintln!("\numbra: Ctrl+C pressed {presses} times, exiting");
        std::process::exit(130);
    }
}

/// Returns true when Ctrl+C was pressed and nothing has handled it yet.
pub fn is_pending() -> bool {
    PENDING.load(Ordering::SeqCst)
}

/// Handles a pending interrupt: returns true and clears it when there was one.
pub fn take() -> bool {
    let was = PENDING.swap(false, Ordering::SeqCst);
    if was {
        PRESSES.store(0, Ordering::SeqCst);
    }
    was
}

/// Sets the flag as if Ctrl+C had been pressed. Used by the unit tests.
#[cfg(test)]
pub fn trigger() {
    PENDING.store(true, Ordering::SeqCst);
}

/// Sleeps for `duration`, waking early when Ctrl+C is pressed. Returns false when it was cut short.
/// @param duration - how long to sleep
pub fn sleep(duration: Duration) -> bool {
    let deadline = Instant::now() + duration;
    loop {
        if is_pending() {
            return false;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return true;
        }
        std::thread::sleep(left.min(POLL));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_stops_on_a_pending_interrupt_and_take_clears_it() {
        trigger();
        let started = Instant::now();
        assert!(!sleep(Duration::from_secs(5)));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(take());
        assert!(!take());
        assert!(sleep(Duration::from_millis(10)));
    }
}

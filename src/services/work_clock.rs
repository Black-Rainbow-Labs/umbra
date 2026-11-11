//! The agent time: how long the agent has spent working in this session. It runs while a prompt
//! is with the agent and stops while Umbra waits for the person, so it adds up across turns and
//! never counts the time the person spends reading or typing.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// An accumulating stopwatch.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkClock {
    accumulated: Duration,
    running_since: Option<Instant>,
}

impl WorkClock {
    /// Starts the clock. Does nothing when it is already running.
    /// @param now - the time to start from
    pub fn start_at(&mut self, now: Instant) {
        if self.running_since.is_none() {
            self.running_since = Some(now);
        }
    }

    /// Stops the clock and adds the running span to the total. Does nothing when it is stopped.
    /// @param now - the time to stop at
    pub fn stop_at(&mut self, now: Instant) {
        if let Some(since) = self.running_since.take() {
            self.accumulated += now.saturating_duration_since(since);
        }
    }

    /// The total so far, including the running span.
    /// @param now - the time to measure at
    pub fn elapsed_at(&self, now: Instant) -> Duration {
        self.accumulated + self.running_since.map_or(Duration::ZERO, |since| now.saturating_duration_since(since))
    }

    /// True while the clock runs.
    pub fn is_running(&self) -> bool {
        self.running_since.is_some()
    }

    /// Sets the total, for a resumed session. The clock is left stopped.
    /// @param total - the time already spent
    pub fn reset_to(&mut self, total: Duration) {
        self.accumulated = total;
        self.running_since = None;
    }
}

/// The session's clock, read by the status bar at every redraw.
static CLOCK: Mutex<WorkClock> = Mutex::new(WorkClock { accumulated: Duration::ZERO, running_since: None });

/// Starts the session's clock: a prompt was handed to the agent, or a question was answered.
pub fn start() {
    CLOCK.lock().unwrap_or_else(|e| e.into_inner()).start_at(Instant::now());
    crate::utils::screen::poke();
}

/// Stops the session's clock: the turn ended, or Umbra is asking the person something.
pub fn stop() {
    CLOCK.lock().unwrap_or_else(|e| e.into_inner()).stop_at(Instant::now());
}

/// The agent time so far.
pub fn elapsed() -> Duration {
    CLOCK.lock().unwrap_or_else(|e| e.into_inner()).elapsed_at(Instant::now())
}

/// True while the agent works.
pub fn is_running() -> bool {
    CLOCK.lock().unwrap_or_else(|e| e.into_inner()).is_running()
}

/// Sets the agent time already spent, for a resumed session.
/// @param seconds - the saved total
pub fn restore(seconds: f64) {
    CLOCK.lock().unwrap_or_else(|e| e.into_inner()).reset_to(Duration::from_secs_f64(seconds.max(0.0)));
}

/// Writes a duration as `m:ss`, or `h:mm:ss` from one hour.
/// @param duration - the duration
pub fn clock_text(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs >= 3600 { format!("{}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60) } else { format!("{}:{:02}", secs / 60, secs % 60) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_while_running_and_adds_up_across_starts() {
        let t0 = Instant::now();
        let mut clock = WorkClock::default();
        clock.start_at(t0);
        clock.start_at(t0 + Duration::from_secs(5));
        assert_eq!(clock.elapsed_at(t0 + Duration::from_secs(10)), Duration::from_secs(10), "a second start changes nothing");
        clock.stop_at(t0 + Duration::from_secs(10));
        assert_eq!(clock.elapsed_at(t0 + Duration::from_secs(60)), Duration::from_secs(10), "stopped, it does not count");
        clock.start_at(t0 + Duration::from_secs(60));
        assert_eq!(clock.elapsed_at(t0 + Duration::from_secs(65)), Duration::from_secs(15));
        assert!(clock.is_running());
    }

    #[test]
    fn writes_minutes_and_seconds() {
        assert_eq!(clock_text(Duration::from_secs(0)), "0:00");
        assert_eq!(clock_text(Duration::from_secs(187)), "3:07");
        assert_eq!(clock_text(Duration::from_secs(3_725)), "1:02:05");
    }
}

//! The weekly usage of a Claude subscription login, for the status bar. One background thread
//! polls the Claude usage endpoint with the token Claude Code saved, at the intervals the
//! endpoint tolerates, and the rate limit headers of each model reply update the value between
//! polls. Sessions that use an API key or a local model show nothing and make no request.
//!
//! The token is only ever sent to `api.anthropic.com`. A provider's `base_url` does not change
//! where the usage request goes, so a config file from a cloned repository cannot send the login
//! token to another host.

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use super::providers::credentials::{anthropic_credential, Credential};
use super::providers::{resolve_model, split_model_spec};
use crate::models::config::UmbraConfig;
use crate::utils::http::{get_text, http_agent};

/// The Claude usage endpoint.
const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

/// The shortest time between two polls; the endpoint answers 429 when polled more often.
const MIN_INTERVAL: Duration = Duration::from_secs(180);

/// The waits after each 429 in a row, in minutes.
const BACKOFF_MINUTES: [u64; 5] = [2, 5, 10, 20, 30];

/// The weekly usage as last read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeeklyUsage {
    /// From 0 to 100.
    pub percent: f32,
    pub resets_at: Option<SystemTime>,
    /// True when the last poll failed and this is an older value.
    pub stale: bool,
    /// The 5 hour window's percent, when the endpoint sent it.
    pub five_hour: Option<f32>,
}

/// The service's state.
struct State {
    /// The access token, while the session uses a Claude login.
    token: Option<String>,
    value: Option<WeeklyUsage>,
    /// When the next poll may run.
    next_poll: Option<Instant>,
    /// When the last poll ran.
    last_poll: Option<Instant>,
    /// The 429s in a row.
    backoff: usize,
    /// Set after an answer that cannot be read: nothing is shown for the rest of the session.
    disabled: bool,
}

static STATE: Mutex<State> = Mutex::new(State { token: None, value: None, next_poll: None, last_poll: None, backoff: 0, disabled: false });
static WAKE: Condvar = Condvar::new();
static STARTED: std::sync::Once = std::sync::Once::new();

/// The weekly usage to show, or None when there is nothing to show.
pub fn current() -> Option<WeeklyUsage> {
    let state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if state.disabled || state.token.is_none() { None } else { state.value }
}

/// Starts the service for a session, or points it at a new model: a Claude login polls, anything
/// else shows nothing. `usage.enabled = false` turns it off.
/// @param config - the merged config
/// @param model - the model the session uses
pub fn start(config: &UmbraConfig, model: &str) {
    let token = if config.usage.enabled { claude_login_token(config, model) } else { None };
    {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let changed = state.token != token;
        state.token = token;
        if changed {
            state.value = None;
            state.disabled = false;
            state.backoff = 0;
            state.next_poll = Some(Instant::now());
        }
    }
    STARTED.call_once(|| {
        std::thread::spawn(poll_loop);
    });
    WAKE.notify_all();
}

/// Asks for a poll after a turn ends, which runs no sooner than 180 seconds after the last one.
pub fn turn_ended() {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let earliest = state.last_poll.map_or(Instant::now(), |at| at + MIN_INTERVAL);
    if state.next_poll.is_none_or(|next| next > earliest) {
        state.next_poll = Some(earliest);
    }
    drop(state);
    WAKE.notify_all();
}

/// Reads the weekly numbers from the rate limit headers of a model reply, when they are there:
/// `anthropic-ratelimit-unified-7d-utilization` (0 to 1) and `...-7d-reset` (epoch seconds).
/// @param headers - the reply's headers
pub fn from_headers(headers: &[(String, String)]) {
    let find = |name: &str| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim().to_string());
    let Some(fraction) = find("anthropic-ratelimit-unified-7d-utilization").and_then(|v| v.parse::<f32>().ok()) else { return };
    let reset = find("anthropic-ratelimit-unified-7d-reset").and_then(|v| v.parse::<u64>().ok()).map(|s| UNIX_EPOCH + Duration::from_secs(s));
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if state.token.is_none() || state.disabled {
        return;
    }
    let five_hour = state.value.and_then(|v| v.five_hour);
    state.value = Some(WeeklyUsage { percent: fraction * 100.0, resets_at: reset.or(state.value.and_then(|v| v.resets_at)), stale: false, five_hour });
}

/// The details line shown when the usage is clicked: the percent, when the week resets, and the
/// 5 hour window.
/// @param usage - the usage
pub fn describe(usage: &WeeklyUsage) -> String {
    let mut text = format!("weekly usage {:.0}%", usage.percent);
    if let Some(at) = usage.resets_at {
        let left = at.duration_since(SystemTime::now()).unwrap_or_default().as_secs();
        text.push_str(&format!(" · resets in {}", days_hours(left)));
    }
    if let Some(five) = usage.five_hour {
        text.push_str(&format!(" · 5 hour window {five:.0}%"));
    }
    if usage.stale {
        text.push_str(" · not updated recently");
    }
    text
}

/// A wait written as days and hours, or hours and minutes under a day.
/// @param secs - the wait in seconds
fn days_hours(secs: u64) -> String {
    if secs >= 86_400 { format!("{}d {}h", secs / 86_400, secs % 86_400 / 3600) } else { format!("{}h {}m", secs / 3600, secs % 3600 / 60) }
}

/// The access token of the Claude login a model uses, or None for an API key, another provider
/// or a missing login.
/// @param config - the merged config
/// @param model - the model spec
fn claude_login_token(config: &UmbraConfig, model: &str) -> Option<String> {
    let resolved = resolve_model(config, model);
    let (provider, _) = split_model_spec(&resolved).ok()?;
    let settings = config.providers.get(provider)?;
    if settings.kind != "anthropic" {
        return None;
    }
    match anthropic_credential(settings).ok()? {
        Credential::OAuthToken(token) => Some(token),
        Credential::ApiKey(_) => None,
    }
}

/// The polling thread: waits until the next poll is due, polls, and schedules the next one from
/// the answer.
fn poll_loop() {
    loop {
        let token = wait_until_due();
        let outcome = poll_once(&token);
        apply_outcome(outcome);
        crate::utils::screen::redraw();
    }
}

/// Waits until a poll is due and returns the token to poll with.
fn wait_until_due() -> String {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        let now = Instant::now();
        match (&state.token, state.next_poll, state.disabled) {
            (Some(token), Some(next), false) if next <= now => {
                let token = token.clone();
                state.next_poll = None;
                state.last_poll = Some(now);
                return token;
            }
            (Some(_), Some(next), false) => state = WAKE.wait_timeout(state, next - now).unwrap_or_else(|e| e.into_inner()).0,
            _ => state = WAKE.wait(state).unwrap_or_else(|e| e.into_inner()),
        }
    }
}

/// What one poll found.
#[derive(Debug, Clone, PartialEq)]
pub enum PollOutcome {
    Value(WeeklyUsage),
    /// 401: the login expired.
    Expired,
    /// 429: polled too often.
    Limited,
    /// A network error or a 5xx.
    Unreachable,
    /// Any other status, or JSON without the expected fields.
    Unreadable,
}

/// Polls the endpoint once.
/// @param token - the access token
fn poll_once(token: &str) -> PollOutcome {
    let url = std::env::var("UMBRA_TEST_USAGE_URL").unwrap_or_else(|_| CLAUDE_USAGE_URL.to_string());
    let headers = vec![("authorization".to_string(), format!("Bearer {token}")), ("anthropic-beta".to_string(), "oauth-2025-04-20".to_string()), ("anthropic-version".to_string(), "2023-06-01".to_string())];
    match get_text(&http_agent(20), &url, &headers) {
        Ok(reply) if reply.status == 200 => parse_usage(&reply.body).map_or(PollOutcome::Unreadable, PollOutcome::Value),
        Ok(reply) if reply.status == 401 => PollOutcome::Expired,
        Ok(reply) if reply.status == 429 => PollOutcome::Limited,
        Ok(reply) if reply.status >= 500 => PollOutcome::Unreachable,
        Ok(_) => PollOutcome::Unreadable,
        Err(_) => PollOutcome::Unreachable,
    }
}

/// Reads the usage endpoint's answer: `seven_day.utilization` (a percent) and
/// `seven_day.resets_at` (ISO 8601), and the same for `five_hour`. None when `seven_day` is
/// missing or null.
/// @param body - the JSON text
pub fn parse_usage(body: &str) -> Option<WeeklyUsage> {
    let json: Value = serde_json::from_str(body).ok()?;
    let week = json.get("seven_day").filter(|v| v.is_object())?;
    let percent = week.get("utilization")?.as_f64()? as f32;
    let resets_at = week.get("resets_at").and_then(Value::as_str).and_then(parse_time);
    let five_hour = json.get("five_hour").and_then(|v| v.get("utilization")).and_then(Value::as_f64).map(|v| v as f32);
    Some(WeeklyUsage { percent, resets_at, stale: false, five_hour })
}

/// Applies a poll's outcome and schedules the next poll.
/// @param outcome - what the poll found
fn apply_outcome(outcome: PollOutcome) {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    match outcome {
        PollOutcome::Value(value) => {
            state.value = Some(value);
            state.backoff = 0;
        }
        PollOutcome::Expired => {
            state.value = None;
            state.next_poll = Some(now + Duration::from_secs(300));
        }
        PollOutcome::Limited => {
            mark_stale(&mut state);
            let minutes = BACKOFF_MINUTES[state.backoff.min(BACKOFF_MINUTES.len() - 1)];
            state.backoff += 1;
            state.next_poll = Some(now + Duration::from_secs(minutes * 60));
        }
        PollOutcome::Unreachable => {
            mark_stale(&mut state);
            state.next_poll = Some(now + Duration::from_secs(60));
        }
        PollOutcome::Unreadable => state.disabled = true,
    }
}

/// Keeps the last value but marks it as old.
/// @param state - the service's state
fn mark_stale(state: &mut State) {
    if let Some(value) = state.value.as_mut() {
        value.stale = true;
    }
}

/// Reads an ISO 8601 time such as `2026-11-14T08:00:00.123+00:00` or `...Z`.
/// @param text - the time
pub fn parse_time(text: &str) -> Option<SystemTime> {
    let (date, rest) = text.split_once('T')?;
    let mut parts = date.split('-').map(|p| p.parse::<i64>().ok());
    let (year, month, day) = (parts.next()??, parts.next()??, parts.next()??);
    let clock: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == ':').collect();
    let mut hms = clock.split(':').map(|p| p.parse::<i64>().ok());
    let (hour, minute, second) = (hms.next()??, hms.next()??, hms.next().flatten().unwrap_or(0));
    let offset = time_offset(&rest[clock.len()..]);
    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + hour * 3600 + minute * 60 + second - offset;
    (secs >= 0).then(|| UNIX_EPOCH + Duration::from_secs(secs as u64))
}

/// The offset from UTC in seconds at the end of an ISO 8601 time: `Z`, `+02:00` or `-0500`,
/// after any fraction of a second.
/// @param tail - the text after the seconds
fn time_offset(tail: &str) -> i64 {
    let tail = tail.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let sign = match tail.chars().next() { Some('+') => 1, Some('-') => -1, _ => return 0 };
    let digits: String = tail[1..].chars().filter(char::is_ascii_digit).collect();
    let hours = digits.get(..2).and_then(|h| h.parse::<i64>().ok()).unwrap_or(0);
    let minutes = digits.get(2..4).and_then(|m| m.parse::<i64>().ok()).unwrap_or(0);
    sign * (hours * 3600 + minutes * 60)
}

/// Days since 1970-01-01 for a date, by Howard Hinnant's civil calendar formula.
/// @param year - the year
/// @param month - the month, 1 to 12
/// @param day - the day of the month
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_weekly_window_and_ignores_unknown_fields() {
        let body = r#"{"five_hour":{"utilization":12.0,"resets_at":"2026-11-10T20:00:00+00:00"},"seven_day":{"utilization":23.5,"resets_at":"2026-11-14T08:00:00.5Z"},"extra_usage":{"x":1}}"#;
        let usage = parse_usage(body).unwrap();
        assert_eq!(usage.percent, 23.5);
        assert_eq!(usage.five_hour, Some(12.0));
        assert_eq!(usage.resets_at.unwrap().duration_since(UNIX_EPOCH).unwrap().as_secs(), 1_794_643_200);
        assert_eq!(parse_usage(r#"{"seven_day":null}"#), None);
        assert_eq!(parse_usage("not json"), None);
    }

    #[test]
    fn reads_times_with_offsets() {
        let utc = parse_time("2026-11-14T08:00:00Z").unwrap();
        assert_eq!(parse_time("2026-11-14T10:00:00+02:00").unwrap(), utc);
        assert_eq!(parse_time("2026-11-14T03:00:00-0500").unwrap(), utc);
        assert_eq!(days_hours(2 * 86_400 + 4 * 3600 + 59), "2d 4h");
    }
}

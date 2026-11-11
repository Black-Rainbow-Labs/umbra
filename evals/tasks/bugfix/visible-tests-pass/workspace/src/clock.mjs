// Clocks used by the limiter. The manual clock lets tests move time forward.

/**
 * Creates a clock that reads the system time in milliseconds.
 */
export function createSystemClock() {
  return { now: () => Date.now() };
}

/**
 * Creates a clock whose time only changes when advance is called.
 * @param startMs - the first time the clock reports
 */
export function createManualClock(startMs = 0) {
  let current = startMs;
  return {
    now: () => current,
    advance: (ms) => { current += ms; },
  };
}

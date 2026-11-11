// Sliding window rate limiter: at most `limit` accepted requests in any `windowMs` span.

import { createSystemClock } from "./clock.mjs";
import { createHitStore } from "./hitStore.mjs";

/**
 * Keeps only the timestamps that still count against the window ending at now.
 * @param timestamps - accepted request times, oldest first
 * @param now - the current time in milliseconds
 * @param windowMs - the window length in milliseconds
 */
function liveHits(timestamps, now, windowMs) {
  const cutoff = now - windowMs;
  return timestamps.filter((timestamp) => timestamp > cutoff);
}

/**
 * Works out how long a rejected caller must wait before a slot frees up.
 * @param hits - the live timestamps, oldest first
 * @param now - the current time in milliseconds
 * @param windowMs - the window length in milliseconds
 */
function millisecondsUntilFree(hits, now, windowMs) {
  const oldest = hits[0];
  return Math.max(0, oldest + windowMs - now);
}

/**
 * Creates a limiter. check(key) records the request when it is allowed.
 * @param options - limit, windowMs and optionally a clock and a store
 */
export function createLimiter({ limit, windowMs, clock = createSystemClock(), store = createHitStore() }) {
  return {
    check(key) {
      const now = clock.now();
      const hits = liveHits(store.list(key), now, windowMs);
      store.replace(key, hits);
      if (hits.length >= limit) {
        return { allowed: false, remaining: 0, retryAfterMs: millisecondsUntilFree(hits, now, windowMs) };
      }
      store.record(key, now);
      return { allowed: true, remaining: limit - hits.length - 1, retryAfterMs: 0 };
    },
  };
}

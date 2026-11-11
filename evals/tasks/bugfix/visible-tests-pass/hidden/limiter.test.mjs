import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const { createLimiter, createManualClock } = await import(pathToFileURL(path.join(process.env.WORKSPACE_DIR, "src", "index.mjs")).href);

test("a request exactly one window after the first is allowed again", () => {
  const clock = createManualClock(0);
  const limiter = createLimiter({ limit: 3, windowMs: 1000, clock });
  limiter.check("a"); limiter.check("a"); limiter.check("a");
  clock.advance(1000);
  const result = limiter.check("a");
  assert.equal(result.allowed, true);
  assert.equal(result.remaining, 2);
});

test("one millisecond before the window ends is still rejected", () => {
  const clock = createManualClock(0);
  const limiter = createLimiter({ limit: 1, windowMs: 1000, clock });
  limiter.check("a");
  clock.advance(999);
  const result = limiter.check("a");
  assert.equal(result.allowed, false);
  assert.equal(result.retryAfterMs, 1);
});

test("retryAfterMs counts from the oldest live request", () => {
  const clock = createManualClock(0);
  const limiter = createLimiter({ limit: 3, windowMs: 1000, clock });
  limiter.check("a");
  clock.advance(300);
  limiter.check("a");
  clock.advance(300);
  limiter.check("a");
  const result = limiter.check("a");
  assert.equal(result.allowed, false);
  assert.equal(result.retryAfterMs, 400);
});

test("waiting retryAfterMs makes the next request succeed", () => {
  const clock = createManualClock(0);
  const limiter = createLimiter({ limit: 2, windowMs: 1000, clock });
  limiter.check("a");
  clock.advance(100);
  limiter.check("a");
  clock.advance(100);
  const rejected = limiter.check("a");
  assert.equal(rejected.retryAfterMs, 800);
  clock.advance(rejected.retryAfterMs);
  assert.equal(limiter.check("a").allowed, true);
});

test("rejected requests do not extend the wait", () => {
  const clock = createManualClock(0);
  const limiter = createLimiter({ limit: 1, windowMs: 1000, clock });
  limiter.check("a");
  clock.advance(400);
  limiter.check("a");
  clock.advance(400);
  assert.equal(limiter.check("a").retryAfterMs, 200);
});

test("basic limit still works", () => {
  const limiter = createLimiter({ limit: 2, windowMs: 1000, clock: createManualClock(0) });
  assert.equal(limiter.check("k").allowed, true);
  assert.equal(limiter.check("k").allowed, true);
  assert.equal(limiter.check("k").allowed, false);
});

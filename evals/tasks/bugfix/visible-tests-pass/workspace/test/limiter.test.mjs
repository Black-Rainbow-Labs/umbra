import test from "node:test";
import assert from "node:assert/strict";
import { createLimiter, createManualClock } from "../src/index.mjs";

test("allows up to the limit and then rejects", () => {
  const clock = createManualClock(0);
  const limiter = createLimiter({ limit: 3, windowMs: 1000, clock });
  assert.equal(limiter.check("a").allowed, true);
  assert.equal(limiter.check("a").allowed, true);
  assert.equal(limiter.check("a").allowed, true);
  assert.equal(limiter.check("a").allowed, false);
});

test("reports how many requests remain", () => {
  const limiter = createLimiter({ limit: 2, windowMs: 1000, clock: createManualClock(0) });
  assert.equal(limiter.check("a").remaining, 1);
  assert.equal(limiter.check("a").remaining, 0);
});

test("keys are independent", () => {
  const limiter = createLimiter({ limit: 1, windowMs: 1000, clock: createManualClock(0) });
  assert.equal(limiter.check("a").allowed, true);
  assert.equal(limiter.check("b").allowed, true);
});

test("allows again once the window has clearly passed", () => {
  const clock = createManualClock(0);
  const limiter = createLimiter({ limit: 1, windowMs: 1000, clock });
  limiter.check("a");
  clock.advance(1500);
  assert.equal(limiter.check("a").allowed, true);
});

test("retryAfterMs for a single request", () => {
  const clock = createManualClock(0);
  const limiter = createLimiter({ limit: 1, windowMs: 1000, clock });
  limiter.check("a");
  clock.advance(250);
  assert.equal(limiter.check("a").retryAfterMs, 750);
});

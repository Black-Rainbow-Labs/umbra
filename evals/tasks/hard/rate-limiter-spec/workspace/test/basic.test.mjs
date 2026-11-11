import test from "node:test";
import assert from "node:assert/strict";
import { RateLimiter } from "../src/rate-limiter.mjs";

test("a new key can use its whole capacity and then is denied", () => {
  let time = 0;
  const limiter = new RateLimiter({ capacity: 2, refillPerSecond: 1, now: () => time });
  assert.equal(limiter.tryConsume("a").allowed, true);
  assert.equal(limiter.tryConsume("a").allowed, true);
  assert.equal(limiter.tryConsume("a").allowed, false);
});

test("tokens come back with time", () => {
  let time = 0;
  const limiter = new RateLimiter({ capacity: 1, refillPerSecond: 1, now: () => time });
  limiter.tryConsume("a");
  time = 1000;
  assert.equal(limiter.tryConsume("a").allowed, true);
});

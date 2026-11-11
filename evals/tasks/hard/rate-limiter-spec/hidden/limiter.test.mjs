import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const { RateLimiter } = await import(pathToFileURL(path.join(process.env.WORKSPACE_DIR, "src", "rate-limiter.mjs")).href);

/**
 * Builds a limiter with a clock the test controls.
 * @param options - limiter options without `now`
 */
function make(options) {
  const clock = { time: 0 };
  const limiter = new RateLimiter({ ...options, now: () => clock.time });
  return { limiter, clock };
}

test("R1 constructor rejects bad options", () => {
  const good = { capacity: 2, refillPerSecond: 1 };
  for (const capacity of [0, 1.5, "3", undefined, -1]) assert.throws(() => new RateLimiter({ ...good, capacity }), (e) => e instanceof RangeError && e.message === "invalid option: capacity");
  for (const refillPerSecond of [0, -1, NaN, Infinity, "1", undefined]) assert.throws(() => new RateLimiter({ ...good, refillPerSecond }), (e) => e instanceof RangeError && e.message === "invalid option: refillPerSecond");
  assert.throws(() => new RateLimiter({ ...good, now: "x" }), (e) => e instanceof RangeError && e.message === "invalid option: now");
  for (const idleTtlMs of [0, -5, NaN, "1000", Infinity]) assert.throws(() => new RateLimiter({ ...good, idleTtlMs }), (e) => e instanceof RangeError && e.message === "invalid option: idleTtlMs");
  assert.throws(() => new RateLimiter(), RangeError);
  assert.doesNotThrow(() => new RateLimiter({ capacity: 1, refillPerSecond: 0.5 }));
});

test("R2 each key has its own full bucket and a denied call takes nothing", () => {
  const { limiter, clock } = make({ capacity: 3, refillPerSecond: 1 });
  assert.deepEqual(limiter.tryConsume("a"), { allowed: true, remaining: 2, retryAfterMs: 0 });
  assert.equal(limiter.tryConsume("a").remaining, 1);
  assert.equal(limiter.tryConsume("a").remaining, 0);
  assert.equal(limiter.tryConsume("a").allowed, false);
  assert.equal(limiter.tryConsume("b").remaining, 2);
  const second = make({ capacity: 5, refillPerSecond: 1 });
  assert.equal(second.limiter.tryConsume("k", 5).remaining, 0);
  second.clock.time = 2000;
  const denied = second.limiter.tryConsume("k", 3);
  assert.equal(denied.allowed, false);
  assert.equal(denied.remaining, 2);
  assert.deepEqual(second.limiter.tryConsume("k", 2), { allowed: true, remaining: 0, retryAfterMs: 0 });
  assert.equal(clock.time, 0);
});

test("R3 refill is continuous and fractions of a token are kept", () => {
  const { limiter, clock } = make({ capacity: 5, refillPerSecond: 2 });
  limiter.tryConsume("a", 5);
  clock.time = 250;
  assert.deepEqual(limiter.tryConsume("a"), { allowed: false, remaining: 0, retryAfterMs: 250 });
  clock.time = 500;
  assert.deepEqual(limiter.tryConsume("a"), { allowed: true, remaining: 0, retryAfterMs: 0 });
  clock.time = 900;
  assert.deepEqual(limiter.tryConsume("a"), { allowed: false, remaining: 0, retryAfterMs: 100 });
  clock.time = 1000;
  assert.equal(limiter.tryConsume("a").allowed, true);
});

test("R4 a bucket never holds more than the capacity", () => {
  const { limiter, clock } = make({ capacity: 3, refillPerSecond: 10 });
  limiter.tryConsume("a", 3);
  clock.time = 60000;
  assert.equal(limiter.peek("a"), 3);
  assert.deepEqual(limiter.tryConsume("a", 3), { allowed: true, remaining: 0, retryAfterMs: 0 });
  assert.equal(limiter.tryConsume("a").allowed, false);
  const other = make({ capacity: 4, refillPerSecond: 10 });
  other.limiter.tryConsume("a");
  other.clock.time = 100;
  assert.equal(other.limiter.peek("a"), 4);
  assert.equal(other.limiter.tryConsume("a", 4).allowed, true);
});

test("R5 retryAfterMs is rounded up and waiting that long is enough", () => {
  for (const [cost, wait] of [[1, 334], [2, 667], [3, 1000], [4, 1334]]) {
    const { limiter, clock } = make({ capacity: 4, refillPerSecond: 3 });
    limiter.tryConsume("a", 4);
    const denied = limiter.tryConsume("a", cost);
    assert.equal(denied.allowed, false);
    assert.equal(denied.retryAfterMs, wait, `cost ${cost}`);
    clock.time = wait;
    assert.equal(limiter.tryConsume("a", cost).allowed, true, `cost ${cost} after ${wait} ms`);
  }
  const { limiter, clock } = make({ capacity: 4, refillPerSecond: 3 });
  limiter.tryConsume("a", 4);
  clock.time = 100;
  assert.equal(limiter.tryConsume("a").retryAfterMs, 234);
});

test("R6 waitTimeMs reads the wait without changing any state", () => {
  const { limiter, clock } = make({ capacity: 2, refillPerSecond: 1 });
  limiter.tryConsume("a", 2);
  assert.equal(limiter.waitTimeMs("a"), 1000);
  assert.equal(limiter.waitTimeMs("a", 2), 2000);
  clock.time = 500;
  assert.equal(limiter.waitTimeMs("a"), 500);
  assert.equal(limiter.waitTimeMs("a", 2), 1500);
  assert.equal(limiter.waitTimeMs("zzz", 2), 0);
  assert.equal(limiter.size(), 1);
  clock.time = 1000;
  assert.equal(limiter.waitTimeMs("a"), 0);
  assert.deepEqual(limiter.tryConsume("a"), { allowed: true, remaining: 0, retryAfterMs: 0 });
  assert.throws(() => limiter.waitTimeMs("a", 3), RangeError);
  assert.throws(() => limiter.waitTimeMs("a", 0), RangeError);
});

test("R7 invalid costs and keys are rejected and create no bucket", () => {
  const { limiter } = make({ capacity: 2, refillPerSecond: 1 });
  for (const cost of [0, -1, 1.5, "2", 3, NaN, null]) assert.throws(() => limiter.tryConsume("a", cost), (e) => e instanceof RangeError && e.message === "invalid cost", `cost ${String(cost)}`);
  for (const key of [5, undefined, null, {}]) {
    assert.throws(() => limiter.tryConsume(key), (e) => e instanceof TypeError && e.message === "key must be a string");
    assert.throws(() => limiter.peek(key), TypeError);
    assert.throws(() => limiter.reset(key), TypeError);
    assert.throws(() => limiter.waitTimeMs(key), TypeError);
  }
  assert.equal(limiter.size(), 0);
  assert.equal(limiter.tryConsume("").allowed, true);
  assert.equal(limiter.size(), 1);
});

test("R8 reset and resetAll give back full buckets", () => {
  const { limiter } = make({ capacity: 2, refillPerSecond: 1 });
  limiter.tryConsume("a", 2);
  limiter.tryConsume("b", 2);
  limiter.reset("a");
  assert.equal(limiter.peek("a"), 2);
  assert.equal(limiter.peek("b"), 0);
  assert.equal(limiter.tryConsume("a").remaining, 1);
  assert.doesNotThrow(() => limiter.reset("never-seen"));
  limiter.resetAll();
  assert.equal(limiter.size(), 0);
  assert.equal(limiter.tryConsume("b").remaining, 1);
});

test("R9 a key idle for idleTtlMs or longer is dropped and starts full again", () => {
  const a = make({ capacity: 10, refillPerSecond: 1, idleTtlMs: 5000 });
  a.limiter.tryConsume("k", 10);
  a.clock.time = 4999;
  assert.equal(a.limiter.peek("k"), 4);
  assert.deepEqual(a.limiter.tryConsume("k"), { allowed: true, remaining: 3, retryAfterMs: 0 });
  a.clock.time = 9998;
  assert.equal(a.limiter.peek("k"), 8);

  const b = make({ capacity: 10, refillPerSecond: 1, idleTtlMs: 5000 });
  b.limiter.tryConsume("k", 10);
  b.limiter.tryConsume("other", 1);
  b.clock.time = 5000;
  assert.equal(b.limiter.peek("k"), 10);
  assert.equal(b.limiter.size(), 0);
  assert.equal(b.limiter.tryConsume("k").remaining, 9);
  assert.equal(b.limiter.size(), 1);
});

test("R9 a denied call refreshes the idle time but peek and waitTimeMs do not", () => {
  const a = make({ capacity: 10, refillPerSecond: 1, idleTtlMs: 5000 });
  a.limiter.tryConsume("k", 10);
  a.clock.time = 3000;
  assert.equal(a.limiter.tryConsume("k", 5).allowed, false);
  a.clock.time = 7999;
  assert.equal(a.limiter.peek("k"), 7);
  a.clock.time = 8000;
  assert.equal(a.limiter.peek("k"), 10);

  const b = make({ capacity: 10, refillPerSecond: 1, idleTtlMs: 5000 });
  b.limiter.tryConsume("k", 10);
  b.clock.time = 3000;
  b.limiter.peek("k");
  b.limiter.waitTimeMs("k", 5);
  b.clock.time = 5000;
  assert.equal(b.limiter.peek("k"), 10);

  const c = make({ capacity: 10, refillPerSecond: 1 });
  c.limiter.tryConsume("k", 10);
  c.clock.time = 1e9;
  assert.equal(c.limiter.size(), 1);
});

test("R10 a clock that moves backwards adds no tokens and becomes the new reference", () => {
  const { limiter, clock } = make({ capacity: 5, refillPerSecond: 1 });
  clock.time = 10000;
  limiter.tryConsume("a", 5);
  clock.time = 5000;
  assert.deepEqual(limiter.tryConsume("a"), { allowed: false, remaining: 0, retryAfterMs: 1000 });
  clock.time = 6000;
  assert.equal(limiter.tryConsume("a").allowed, true);
});

test("R11 without a now option the limiter uses Date.now", () => {
  const limiter = new RateLimiter({ capacity: 1, refillPerSecond: 1 });
  assert.equal(limiter.tryConsume("a").allowed, true);
  const denied = limiter.tryConsume("a");
  assert.equal(denied.allowed, false);
  assert.ok(denied.retryAfterMs >= 1 && denied.retryAfterMs <= 1000, `retryAfterMs ${denied.retryAfterMs}`);
});

test("R12 results are new plain objects and remaining is a whole number", () => {
  const { limiter, clock } = make({ capacity: 5, refillPerSecond: 1.5 });
  const first = limiter.tryConsume("a");
  assert.deepEqual(Object.keys(first).sort(), ["allowed", "remaining", "retryAfterMs"]);
  first.remaining = 99;
  const second = limiter.tryConsume("a");
  assert.notEqual(first, second);
  assert.equal(second.remaining, 3);
  clock.time = 1000;
  const third = limiter.tryConsume("a");
  assert.equal(third.remaining, 3);
  assert.ok(Number.isInteger(third.remaining));
});

// Hidden checks that code added after the broken commit and built on roundCents still rounds toward zero.
// WORKSPACE_DIR is the repository after the fix.

import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const load = (relative) => import(pathToFileURL(path.join(process.env.WORKSPACE_DIR, relative)).href);

test("roundCents rounds toward zero", async () => {
  const { roundCents } = await load("src/money.mjs");
  assert.equal(roundCents(2.9), 2);
  assert.equal(roundCents(0.99), 0);
  assert.equal(roundCents(100.5), 100);
});

test("late fees never charge a fraction of a cent", async () => {
  const { lateFee } = await load("src/latefee.mjs");
  assert.equal(lateFee(999, 2, 25), 4);
  assert.equal(lateFee(10000, 3, 50), 150);
  assert.equal(lateFee(333, 1, 100), 3);
  assert.equal(lateFee(1999, 7, 15), 20);
  assert.equal(lateFee(500, 0, 50), 0);
});

test("split payments add up to the total", async () => {
  const { splitCents } = await load("src/split.mjs");
  assert.deepEqual(splitCents(100, 3), [34, 33, 33]);
  assert.deepEqual(splitCents(200, 3), [67, 67, 66]);
  assert.deepEqual(splitCents(5, 2), [3, 2]);
  assert.deepEqual(splitCents(1000, 7), [143, 143, 143, 143, 143, 143, 142]);
  for (const [total, parts] of [[1, 3], [10, 4], [999, 8], [12345, 11]]) {
    const split = splitCents(total, parts);
    assert.equal(split.reduce((a, b) => a + b, 0), total);
    assert.equal(split.length, parts);
    assert.ok(Math.max(...split) - Math.min(...split) <= 1);
  }
});

test("refunds never refund a fraction of a cent", async () => {
  const { refundCents } = await load("src/refunds.mjs");
  assert.equal(refundCents(999, 50), 499);
  assert.equal(refundCents(1001, 50), 500);
  assert.equal(refundCents(1, 50), 0);
  assert.equal(refundCents(333, 10), 33);
  assert.equal(refundCents(1999, 15), 299);
});

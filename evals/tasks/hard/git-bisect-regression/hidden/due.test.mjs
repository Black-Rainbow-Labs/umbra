// Hidden due date behavior checks. WORKSPACE_DIR is the repository after the fix.

import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const load = (relative) => import(pathToFileURL(path.join(process.env.WORKSPACE_DIR, relative)).href);

test("dueDate counts calendar days by default", async () => {
  const { dueDate } = await load("src/dates.mjs");
  assert.equal(dueDate("2024-01-30", 30), "2024-02-29");
  assert.equal(dueDate("2023-12-15", 20), "2024-01-04");
  assert.equal(dueDate("2024-03-01", 1), "2024-03-02");
  assert.equal(dueDate("2024-03-02", 2), "2024-03-04");
  assert.equal(dueDate("2024-02-29", 0), "2024-02-29");
  assert.equal(dueDate("2024-12-30", 3), "2025-01-02");
});

test("dueDate counts calendar days when the option is false or empty", async () => {
  const { dueDate } = await load("src/dates.mjs");
  assert.equal(dueDate("2024-03-01", 2, { skipWeekends: false }), "2024-03-03");
  assert.equal(dueDate("2024-03-01", 2, {}), "2024-03-03");
  assert.equal(dueDate("2024-03-01", 2, { other: true }), "2024-03-03");
});

test("dueDate skips weekends only when asked", async () => {
  const { dueDate } = await load("src/dates.mjs");
  assert.equal(dueDate("2024-02-02", 5, { skipWeekends: true }), "2024-02-09");
  assert.equal(dueDate("2024-03-02", 1, { skipWeekends: true }), "2024-03-04");
  assert.equal(dueDate("2024-03-01", 1, { skipWeekends: true }), "2024-03-04");
  assert.equal(dueDate("2024-03-01", 10, { skipWeekends: true }), "2024-03-15");
  assert.equal(dueDate("2024-03-03", 0, { skipWeekends: true }), "2024-03-03");
});

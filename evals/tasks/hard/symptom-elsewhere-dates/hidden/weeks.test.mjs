import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const workspace = process.env.WORKSPACE_DIR;
const load = (file) => import(pathToFileURL(path.join(workspace, "src", file)).href);
const { startOfWeek, addDays, formatDate, parseTimestamp } = await load("dates.mjs");
const { groupBy } = await load("group.mjs");
const { recentWeeks } = await load("range.mjs");
const { weeklyTotals, weeklyTotalsFor, renderWeeklyReport } = await load("report.mjs");

const iso = (date) => date.toISOString();
const order = (id, placedAt, amount = 100) => ({ id, placedAt, amount });

test("startOfWeek returns Monday 00:00 UTC for every day of a week", () => {
  const monday = "2024-03-04T00:00:00.000Z";
  const samples = ["2024-03-04T00:00:00Z", "2024-03-05T13:45:10Z", "2024-03-06T23:59:59.999Z", "2024-03-07T00:00:00Z", "2024-03-08T17:00:00Z", "2024-03-09T03:00:00Z", "2024-03-10T00:00:00Z", "2024-03-10T23:59:59.999Z"];
  for (const sample of samples) assert.equal(iso(startOfWeek(new Date(sample))), monday, sample);
  assert.equal(iso(startOfWeek(new Date("2024-03-11T00:00:00Z"))), "2024-03-11T00:00:00.000Z");
});

test("startOfWeek handles month, year and leap day boundaries", () => {
  assert.equal(iso(startOfWeek(new Date("2024-02-29T08:00:00Z"))), "2024-02-26T00:00:00.000Z");
  assert.equal(iso(startOfWeek(new Date("2025-01-05T10:00:00Z"))), "2024-12-30T00:00:00.000Z");
  assert.equal(iso(startOfWeek(new Date("2025-01-01T00:00:00Z"))), "2024-12-30T00:00:00.000Z");
  assert.equal(iso(startOfWeek(new Date("2024-06-02T23:00:00Z"))), "2024-05-27T00:00:00.000Z");
});

test("startOfWeek does not change its argument", () => {
  const input = new Date("2024-03-10T12:00:00Z");
  startOfWeek(input);
  assert.equal(iso(input), "2024-03-10T12:00:00.000Z");
});

test("addDays adds whole UTC days across daylight saving changes and does not mutate", () => {
  const input = new Date("2024-03-09T00:00:00Z");
  assert.equal(iso(addDays(input, 2)), "2024-03-11T00:00:00.000Z");
  assert.equal(iso(input), "2024-03-09T00:00:00.000Z");
  assert.equal(iso(addDays(new Date("2024-04-06T00:00:00Z"), 2)), "2024-04-08T00:00:00.000Z");
  assert.equal(iso(addDays(new Date("2024-11-03T12:00:00Z"), -7)), "2024-10-27T12:00:00.000Z");
  assert.equal(iso(addDays(new Date("2024-12-30T00:00:00Z"), 7)), "2025-01-06T00:00:00.000Z");
});

test("formatDate prints the UTC calendar date", () => {
  assert.equal(formatDate(new Date("2024-03-10T23:30:00Z")), "2024-03-10");
  assert.equal(formatDate(new Date("2024-03-11T00:30:00Z")), "2024-03-11");
  assert.equal(formatDate(new Date("2024-01-05T00:00:00Z")), "2024-01-05");
  assert.equal(formatDate(new Date("2024-12-31T23:59:59.999Z")), "2024-12-31");
});

test("parseTimestamp respects offsets and rejects bad text", () => {
  assert.equal(iso(parseTimestamp("2024-03-10T22:30:00-05:00")), "2024-03-11T03:30:00.000Z");
  assert.equal(iso(parseTimestamp("2024-03-11T01:00:00+02:00")), "2024-03-10T23:00:00.000Z");
  assert.throws(() => parseTimestamp("not a date"), /invalid timestamp/);
});

test("weeklyTotals puts orders in the week of their UTC instant", () => {
  const rows = weeklyTotals([
    order(1, "2024-03-10T23:30:00Z", 100),
    order(2, "2024-03-11T00:00:00Z", 200),
    order(3, "2024-03-17T23:59:59.999Z", 400),
    order(4, "2024-03-04T00:00:00Z", 800),
    order(5, "2024-03-10T22:30:00-05:00", 1600),
    order(6, "2024-03-11T01:00:00+02:00", 3200),
  ]);
  assert.deepEqual(rows, [
    { week: "2024-03-04", orders: 3, total: 100 + 800 + 3200 },
    { week: "2024-03-11", orders: 3, total: 200 + 400 + 1600 },
  ]);
});

test("weeklyTotals sorts weeks ascending across a year boundary", () => {
  const rows = weeklyTotals([order(1, "2025-01-08T10:00:00Z"), order(2, "2024-12-29T10:00:00Z"), order(3, "2024-12-31T10:00:00Z")]);
  assert.deepEqual(rows.map((row) => row.week), ["2024-12-23", "2024-12-30", "2025-01-06"]);
  assert.deepEqual(rows.map((row) => row.orders), [1, 1, 1]);
});

test("weeklyTotals results do not depend on earlier calls", () => {
  const first = weeklyTotals([order(1, "2024-03-05T10:00:00Z", 500), order(2, "2024-03-06T10:00:00Z", 500)]);
  const second = weeklyTotals([order(3, "2024-03-12T10:00:00Z", 70)]);
  const third = weeklyTotals([order(1, "2024-03-05T10:00:00Z", 500), order(2, "2024-03-06T10:00:00Z", 500)]);
  assert.deepEqual(first, [{ week: "2024-03-04", orders: 2, total: 1000 }]);
  assert.deepEqual(second, [{ week: "2024-03-11", orders: 1, total: 70 }]);
  assert.deepEqual(third, first);
  assert.deepEqual(weeklyTotals([]), []);
});

test("groupBy returns a new Map each call in first seen key order", () => {
  const a = groupBy([1, 2, 3, 4, 5], (n) => (n % 2 ? "odd" : "even"));
  const b = groupBy([10], () => "ten");
  assert.deepEqual([...a.keys()], ["odd", "even"]);
  assert.deepEqual(a.get("odd"), [1, 3, 5]);
  assert.deepEqual([...b.keys()], ["ten"]);
  assert.notEqual(a, b);
  assert.equal(groupBy([], (x) => x).size, 0);
});

test("recentWeeks lists Monday keys oldest first across daylight saving and year changes", () => {
  assert.deepEqual(recentWeeks(new Date("2024-03-20T12:00:00Z"), 4), ["2024-02-26", "2024-03-04", "2024-03-11", "2024-03-18"]);
  assert.deepEqual(recentWeeks(new Date("2024-03-17T23:59:59Z"), 2), ["2024-03-04", "2024-03-11"]);
  assert.deepEqual(recentWeeks(new Date("2024-03-11T00:00:00Z"), 2), ["2024-03-04", "2024-03-11"]);
  assert.deepEqual(recentWeeks(new Date("2025-01-02T09:00:00Z"), 3), ["2024-12-16", "2024-12-23", "2024-12-30"]);
  assert.deepEqual(recentWeeks(new Date("2024-11-06T09:00:00Z"), 3), ["2024-10-21", "2024-10-28", "2024-11-04"]);
  assert.deepEqual(recentWeeks(new Date("2024-03-20T12:00:00Z"), 0), []);
});

test("weeklyTotalsFor fills empty weeks and ignores orders outside the range", () => {
  const rows = weeklyTotalsFor([
    order(1, "2024-03-05T10:00:00Z", 300),
    order(2, "2024-03-10T23:30:00Z", 200),
    order(3, "2024-03-19T10:00:00Z", 50),
    order(4, "2024-01-01T10:00:00Z", 9999),
    order(5, "2024-04-02T10:00:00Z", 9999),
  ], new Date("2024-03-20T12:00:00Z"), 4);
  assert.deepEqual(rows, [
    { week: "2024-02-26", orders: 0, total: 0 },
    { week: "2024-03-04", orders: 2, total: 500 },
    { week: "2024-03-11", orders: 0, total: 0 },
    { week: "2024-03-18", orders: 1, total: 50 },
  ]);
});

test("renderWeeklyReport prints weeks ascending with singular and plural nouns", () => {
  const text = renderWeeklyReport([order(1, "2024-03-12T10:00:00Z", 705), order(2, "2024-03-10T23:30:00Z", 1000), order(3, "2024-03-05T10:00:00Z", 250)]);
  assert.equal(text, "Week of 2024-03-04: 2 orders, 12.50\nWeek of 2024-03-11: 1 order, 7.05\n");
  assert.equal(renderWeeklyReport([]), "");
});

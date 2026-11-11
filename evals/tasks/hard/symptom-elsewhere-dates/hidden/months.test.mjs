import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const workspace = process.env.WORKSPACE_DIR;
const load = (file) => import(pathToFileURL(path.join(workspace, "src", file)).href);
const { startOfWeek, addDays, formatDate, parseTimestamp } = await load("dates.mjs");
const { recentWeeks } = await load("range.mjs");
const { weeklyTotals, weeklyTotalsFor, renderWeeklyReport } = await load("report.mjs");
const { startOfMonth, daysInMonth, monthlyTotals } = await load("month.mjs");
const { weekNumber, weekRangeLabel } = await load("labels.mjs");
const lib = await load("index.mjs");

const iso = (date) => date.toISOString();
const order = (id, placedAt, amount = 100) => ({ id, placedAt, amount });
const MONTH_NAMES = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/**
 * Returns the Monday keys of a number of consecutive weeks starting at the given Monday, computed with UTC arithmetic.
 * @param first - the first Monday as a UTC millisecond value
 * @param count - how many weeks
 */
function mondays(first, count) {
  return Array.from({ length: count }, (_, i) => new Date(first + i * 7 * 86400000).toISOString().slice(0, 10));
}

test("parseTimestamp reads text without an offset as UTC", () => {
  assert.equal(iso(parseTimestamp("2024-03-10T22:30:00")), "2024-03-10T22:30:00.000Z");
  assert.equal(iso(parseTimestamp("2024-03-10T22:30")), "2024-03-10T22:30:00.000Z");
  assert.equal(iso(parseTimestamp("2024-03-10T22:30:15.250")), "2024-03-10T22:30:15.250Z");
  assert.equal(iso(parseTimestamp("2024-03-10")), "2024-03-10T00:00:00.000Z");
  assert.equal(iso(parseTimestamp("2024-03-10T22:30:00+00:00")), "2024-03-10T22:30:00.000Z");
  assert.throws(() => parseTimestamp("2024-13-45T00:00:00"), /invalid timestamp/);
});

test("startOfMonth returns the first of the UTC month at midnight UTC and does not mutate", () => {
  const input = new Date("2024-03-31T23:30:00Z");
  assert.equal(iso(startOfMonth(input)), "2024-03-01T00:00:00.000Z");
  assert.equal(iso(input), "2024-03-31T23:30:00.000Z");
  assert.equal(iso(startOfMonth(new Date("2024-04-01T00:00:00Z"))), "2024-04-01T00:00:00.000Z");
  assert.equal(iso(startOfMonth(new Date("2024-12-31T23:59:59.999Z"))), "2024-12-01T00:00:00.000Z");
  assert.equal(iso(startOfMonth(new Date("2025-01-01T00:00:00Z"))), "2025-01-01T00:00:00.000Z");
});

test("daysInMonth uses the UTC month, including leap years and month ends", () => {
  const expected = [["2024-01-15T00:00:00Z", 31], ["2024-02-15T00:00:00Z", 29], ["2023-02-15T00:00:00Z", 28], ["2100-02-10T00:00:00Z", 28], ["2000-02-10T00:00:00Z", 29],
    ["2024-03-31T23:30:00Z", 31], ["2024-04-01T00:30:00Z", 30], ["2024-04-30T23:59:59.999Z", 30], ["2024-05-01T00:00:00Z", 31], ["2024-12-31T23:59:59Z", 31], ["2024-06-30T12:00:00Z", 30]];
  for (const [text, days] of expected) assert.equal(daysInMonth(new Date(text)), days, text);
});

test("monthlyTotals puts orders in the UTC month of their instant, sorted, with no state between calls", () => {
  const rows = monthlyTotals([
    order(1, "2024-03-31T23:30:00Z", 100),
    order(2, "2024-04-01T00:00:00Z", 200),
    order(3, "2024-03-31T22:30:00-05:00", 400),
    order(4, "2024-04-01T01:00:00+02:00", 800),
    order(5, "2023-12-31T23:59:59.999Z", 1600),
    order(6, "2024-01-01T00:00:00", 3200),
  ]);
  assert.deepEqual(rows, [
    { month: "2023-12", orders: 1, total: 1600 },
    { month: "2024-01", orders: 1, total: 3200 },
    { month: "2024-03", orders: 2, total: 100 + 800 },
    { month: "2024-04", orders: 2, total: 200 + 400 },
  ]);
  assert.deepEqual(monthlyTotals([order(7, "2024-05-05T00:00:00Z", 5)]), [{ month: "2024-05", orders: 1, total: 5 }]);
  assert.deepEqual(monthlyTotals([]), []);
});

test("weekNumber gives the ISO 8601 week of the UTC day", () => {
  const expected = [["2024-01-01T00:00:00Z", "2024-W01"], ["2024-03-10T23:30:00Z", "2024-W10"], ["2024-03-11T00:00:00Z", "2024-W11"], ["2024-12-29T23:59:59Z", "2024-W52"],
    ["2024-12-30T00:00:00Z", "2025-W01"], ["2025-01-05T12:00:00Z", "2025-W01"], ["2021-01-03T10:00:00Z", "2020-W53"], ["2021-01-04T00:00:00Z", "2021-W01"],
    ["2023-01-01T08:00:00Z", "2022-W52"], ["2020-12-31T08:00:00Z", "2020-W53"], ["2026-12-31T08:00:00Z", "2026-W53"], ["2027-01-03T23:00:00Z", "2026-W53"],
    ["2024-07-04T12:00:00Z", "2024-W27"], ["2024-02-29T00:00:00Z", "2024-W09"], ["2015-12-31T00:00:00Z", "2015-W53"], ["2016-01-03T23:59:59Z", "2015-W53"]];
  for (const [text, week] of expected) assert.equal(weekNumber(new Date(text)), week, text);
  const input = new Date("2024-03-10T12:00:00Z");
  weekNumber(input);
  assert.equal(iso(input), "2024-03-10T12:00:00.000Z");
});

test("weekRangeLabel formats the Monday to Sunday range of a week", () => {
  assert.equal(weekRangeLabel("2024-03-04"), "Mar 4 - Mar 10, 2024");
  assert.equal(weekRangeLabel("2024-02-26"), "Feb 26 - Mar 3, 2024");
  assert.equal(weekRangeLabel("2024-03-25"), "Mar 25 - Mar 31, 2024");
  assert.equal(weekRangeLabel("2024-04-01"), "Apr 1 - Apr 7, 2024");
  assert.equal(weekRangeLabel("2024-12-30"), "Dec 30, 2024 - Jan 5, 2025");
  assert.equal(weekRangeLabel("2018-12-31"), "Dec 31, 2018 - Jan 6, 2019");
  assert.equal(weekRangeLabel("2024-12-23"), "Dec 23 - Dec 29, 2024");
});

test("weekRangeLabel rejects anything that is not the date of a Monday", () => {
  for (const bad of ["2024-03-05", "2024-03-03", "2024-3-4", "", "abc", "2024-02-30", "2024-13-01", "2024-03-04T00:00:00Z", " 2024-03-04", undefined]) {
    assert.throws(() => weekRangeLabel(bad), (error) => error instanceof Error && error.message === `invalid week: ${bad}`, String(bad));
  }
});

test("weekRangeLabel agrees with independent UTC arithmetic for 130 consecutive weeks", () => {
  const first = Date.UTC(2023, 0, 2);
  mondays(first, 130).forEach((key, index) => {
    const start = new Date(first + index * 7 * 86400000);
    const end = new Date(start.getTime() + 6 * 86400000);
    const left = `${MONTH_NAMES[start.getUTCMonth()]} ${start.getUTCDate()}`;
    const right = `${MONTH_NAMES[end.getUTCMonth()]} ${end.getUTCDate()}`;
    const label = start.getUTCFullYear() === end.getUTCFullYear() ? `${left} - ${right}, ${end.getUTCFullYear()}` : `${left}, ${start.getUTCFullYear()} - ${right}, ${end.getUTCFullYear()}`;
    assert.equal(weekRangeLabel(key), label, key);
  });
});

test("weeklyTotals gives the right week for one order every 6 hours through a leap year", () => {
  const orders = [];
  const start = Date.UTC(2024, 0, 1);
  for (let i = 0; i < 366 * 4; i += 1) orders.push(order(i, new Date(start + i * 6 * 3600000).toISOString(), 1));
  const rows = weeklyTotals(orders);
  assert.equal(rows.length, 53);
  assert.deepEqual(rows.map((row) => row.week), mondays(Date.UTC(2024, 0, 1), 53));
  assert.deepEqual(rows.slice(0, 52).map((row) => row.orders), Array(52).fill(28));
  assert.deepEqual(rows[52], { week: "2024-12-30", orders: 8, total: 8 });
});

test("recentWeeks lists 60 consecutive Mondays across daylight saving changes in any zone", () => {
  for (const now of ["2024-12-31T23:30:00Z", "2024-04-07T14:30:00Z", "2024-09-29T13:30:00Z", "2024-03-31T00:30:00Z"]) {
    const date = new Date(now);
    const last = Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate() - ((date.getUTCDay() + 6) % 7));
    assert.deepEqual(recentWeeks(date, 60), mondays(last - 59 * 7 * 86400000, 60), now);
  }
});

test("weeklyTotalsFor counts orders placed without an offset in the UTC week", () => {
  const rows = weeklyTotalsFor([order(1, "2024-03-10T23:30:00", 100), order(2, "2024-03-11T00:30:00", 200), order(3, "2024-03-03", 5)], new Date("2024-03-12T12:00:00Z"), 3);
  assert.deepEqual(rows, [
    { week: "2024-02-26", orders: 1, total: 5 },
    { week: "2024-03-04", orders: 1, total: 100 },
    { week: "2024-03-11", orders: 1, total: 200 },
  ]);
  assert.equal(renderWeeklyReport([order(1, "2024-03-10T23:30:00", 150)]), "Week of 2024-03-04: 1 order, 1.50\n");
});

test("no helper changes the Date it is given", () => {
  const input = new Date("2024-03-10T12:34:56.789Z");
  const stamp = input.getTime();
  startOfWeek(input);
  addDays(input, 3);
  formatDate(input);
  startOfMonth(input);
  daysInMonth(input);
  weekNumber(input);
  recentWeeks(input, 3);
  assert.equal(input.getTime(), stamp);
});

test("index exports the new helpers next to the old ones", () => {
  for (const name of ["weeklyTotals", "weeklyTotalsFor", "renderWeeklyReport", "recentWeeks", "parseTimestamp", "startOfWeek", "addDays", "formatDate", "startOfMonth", "daysInMonth", "monthlyTotals", "weekNumber", "weekRangeLabel", "groupBy"]) {
    assert.equal(typeof lib[name], "function", name);
  }
});

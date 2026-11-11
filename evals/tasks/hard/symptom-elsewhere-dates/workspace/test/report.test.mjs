import test from "node:test";
import assert from "node:assert/strict";
import { weeklyTotals, renderWeeklyReport } from "../src/report.mjs";

const orders = [
  { id: 1, placedAt: "2024-03-05T10:00:00Z", amount: 1000 },
  { id: 2, placedAt: "2024-03-06T12:30:00Z", amount: 2500 },
  { id: 3, placedAt: "2024-03-12T09:00:00Z", amount: 700 },
];

test("weeklyTotals groups orders by Monday week", () => {
  assert.deepEqual(weeklyTotals(orders), [
    { week: "2024-03-04", orders: 2, total: 3500 },
    { week: "2024-03-11", orders: 1, total: 700 },
  ]);
});

test("weeklyTotals puts a Sunday order in the week that is ending", () => {
  const rows = weeklyTotals([{ id: 4, placedAt: "2024-03-10T12:00:00Z", amount: 100 }]);
  assert.deepEqual(rows, [{ week: "2024-03-04", orders: 1, total: 100 }]);
});

test("weeklyTotals gives the same answer when called twice", () => {
  assert.deepEqual(weeklyTotals(orders), weeklyTotals(orders));
});

test("renderWeeklyReport prints one line per week", () => {
  assert.equal(renderWeeklyReport(orders), "Week of 2024-03-04: 2 orders, 35.00\nWeek of 2024-03-11: 1 order, 7.00\n");
});

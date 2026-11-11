import { groupBy } from "./group.mjs";
import { parseTimestamp, startOfWeek, formatDate } from "./dates.mjs";
import { recentWeeks } from "./range.mjs";

/**
 * Adds up the amounts (whole cents) of a list of orders.
 * @param orders - orders with an integer `amount`
 */
function sumAmounts(orders) {
  return orders.reduce((sum, order) => sum + order.amount, 0);
}

/**
 * Totals orders per week. Returns rows { week, orders, total } sorted by week ascending, where
 * week is the YYYY-MM-DD of the Monday, orders is the count and total is the sum in cents.
 * @param orders - orders with `placedAt` (ISO 8601 text) and `amount` (integer cents)
 */
export function weeklyTotals(orders) {
  const groups = groupBy(orders, (order) => formatDate(startOfWeek(parseTimestamp(order.placedAt))));
  const rows = [];
  for (const [week, list] of groups) rows.push({ week, orders: list.length, total: sumAmounts(list) });
  return rows.sort((a, b) => (a.week < b.week ? -1 : a.week > b.week ? 1 : 0));
}

/**
 * Totals orders for each of the last `count` weeks ending with the week of `now`, including
 * weeks without orders as { week, orders: 0, total: 0 }. Orders outside those weeks are ignored.
 * @param orders - orders with `placedAt` and `amount`
 * @param now - a Date inside the newest week
 * @param count - how many weeks to cover
 */
export function weeklyTotalsFor(orders, now, count) {
  const byWeek = new Map(weeklyTotals(orders).map((row) => [row.week, row]));
  return recentWeeks(now, count).map((week) => byWeek.get(week) ?? { week, orders: 0, total: 0 });
}

/**
 * Renders one text line per week: "Week of 2024-03-04: 3 orders, 45.00".
 * @param orders - orders with `placedAt` and `amount`
 */
export function renderWeeklyReport(orders) {
  const lines = weeklyTotals(orders).map((row) => {
    const noun = row.orders === 1 ? "order" : "orders";
    return `Week of ${row.week}: ${row.orders} ${noun}, ${(row.total / 100).toFixed(2)}`;
  });
  return lines.join("\n") + (lines.length ? "\n" : "");
}

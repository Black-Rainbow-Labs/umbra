import { groupBy } from "./group.mjs";
import { parseTimestamp } from "./dates.mjs";

/**
 * Returns midnight at the start of the first day of the month that contains the given date.
 * @param date - a Date
 */
export function startOfMonth(date) {
  return new Date(date.getFullYear(), date.getMonth(), 1);
}

/**
 * Returns how many days the month that contains the given date has.
 * @param date - a Date
 */
export function daysInMonth(date) {
  return new Date(date.getFullYear(), date.getMonth() + 1, 0).getDate();
}

/**
 * Totals orders per month. Returns rows { month, orders, total } sorted by month ascending, where
 * month is YYYY-MM, orders is the count and total is the sum in cents.
 * @param orders - orders with `placedAt` (ISO 8601 text) and `amount` (integer cents)
 */
export function monthlyTotals(orders) {
  const groups = groupBy(orders, (order) => {
    const date = parseTimestamp(order.placedAt);
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}`;
  });
  const rows = [];
  for (const [month, list] of groups) rows.push({ month, orders: list.length, total: list.reduce((sum, order) => sum + order.amount, 0) });
  return rows.sort((a, b) => (a.month < b.month ? -1 : a.month > b.month ? 1 : 0));
}

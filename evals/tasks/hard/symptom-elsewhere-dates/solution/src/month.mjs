import { groupBy } from "./group.mjs";
import { parseTimestamp, formatDate } from "./dates.mjs";

/**
 * Returns midnight UTC at the start of the first day of the (UTC) month that contains the given date.
 * @param date - a Date
 */
export function startOfMonth(date) {
  return new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), 1));
}

/**
 * Returns how many days the (UTC) month that contains the given date has.
 * @param date - a Date
 */
export function daysInMonth(date) {
  return new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth() + 1, 0)).getUTCDate();
}

/**
 * Totals orders per UTC month. Returns rows { month, orders, total } sorted by month ascending, where
 * month is YYYY-MM, orders is the count and total is the sum in cents.
 * @param orders - orders with `placedAt` (ISO 8601 text) and `amount` (integer cents)
 */
export function monthlyTotals(orders) {
  const groups = groupBy(orders, (order) => formatDate(startOfMonth(parseTimestamp(order.placedAt))).slice(0, 7));
  const rows = [];
  for (const [month, list] of groups) rows.push({ month, orders: list.length, total: list.reduce((sum, order) => sum + order.amount, 0) });
  return rows.sort((a, b) => (a.month < b.month ? -1 : a.month > b.month ? 1 : 0));
}

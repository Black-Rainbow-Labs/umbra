import { startOfWeek, addDays, formatDate } from "./dates.mjs";

/**
 * Lists the week keys (the YYYY-MM-DD of each Monday) of the last `count` weeks, oldest first,
 * ending with the week that contains `now`.
 * @param now - a Date inside the newest week
 * @param count - how many weeks to list
 */
export function recentWeeks(now, count) {
  const newest = startOfWeek(now);
  const weeks = [];
  for (let back = count - 1; back >= 0; back -= 1) {
    weeks.push(formatDate(addDays(newest, -7 * back)));
  }
  return weeks;
}

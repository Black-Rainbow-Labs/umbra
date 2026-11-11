import { addDays } from "./dates.mjs";

const DAY_FORMAT = { month: "short", day: "numeric" };

/**
 * Returns the ISO 8601 week of a date as "YYYY-Www", for example "2024-W10".
 * @param date - a Date
 */
export function weekNumber(date) {
  const jan1 = new Date(date.getFullYear(), 0, 1);
  const dayOfYear = Math.floor((date - jan1) / 86400000);
  const week = Math.ceil((dayOfYear + jan1.getDay() + 1) / 7);
  return `${date.getFullYear()}-W${String(week).padStart(2, "0")}`;
}

/**
 * Returns a label for the week that starts on the given Monday, for example "Mar 4 - Mar 10, 2024".
 * @param weekKey - the YYYY-MM-DD of the Monday
 */
export function weekRangeLabel(weekKey) {
  const start = new Date(weekKey);
  const end = addDays(start, 6);
  return `${start.toLocaleDateString("en-US", DAY_FORMAT)} - ${end.toLocaleDateString("en-US", DAY_FORMAT)}, ${end.getFullYear()}`;
}

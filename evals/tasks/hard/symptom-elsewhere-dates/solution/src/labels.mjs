import { addDays, startOfWeek } from "./dates.mjs";

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const DAY_MS = 24 * 60 * 60 * 1000;

/**
 * Returns the ISO 8601 week of a date (by its UTC day) as "YYYY-Www", for example "2024-W10".
 * The week belongs to the year that contains its Thursday.
 * @param date - a Date
 */
export function weekNumber(date) {
  const thursday = addDays(startOfWeek(date), 3);
  const year = thursday.getUTCFullYear();
  const dayOfYear = Math.round((thursday.getTime() - Date.UTC(year, 0, 1)) / DAY_MS);
  return `${year}-W${String(Math.floor(dayOfYear / 7) + 1).padStart(2, "0")}`;
}

/**
 * Formats the UTC month and day of a Date as "Mar 4".
 * @param date - a Date
 */
function monthDay(date) {
  return `${MONTHS[date.getUTCMonth()]} ${date.getUTCDate()}`;
}

/**
 * Returns a label for the week that starts on the given Monday: "Mar 4 - Mar 10, 2024", or
 * "Dec 30, 2024 - Jan 5, 2025" when the week crosses a year. Throws Error("invalid week: <value>")
 * when the text is not the YYYY-MM-DD of a real Monday.
 * @param weekKey - the YYYY-MM-DD of the Monday
 */
export function weekRangeLabel(weekKey) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(weekKey));
  const start = match ? new Date(Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3]))) : null;
  const real = start && start.getUTCFullYear() === Number(match[1]) && start.getUTCMonth() === Number(match[2]) - 1 && start.getUTCDate() === Number(match[3]);
  if (!real || start.getUTCDay() !== 1) throw new Error(`invalid week: ${weekKey}`);
  const end = addDays(start, 6);
  if (start.getUTCFullYear() !== end.getUTCFullYear()) return `${monthDay(start)}, ${start.getUTCFullYear()} - ${monthDay(end)}, ${end.getUTCFullYear()}`;
  return `${monthDay(start)} - ${monthDay(end)}, ${end.getUTCFullYear()}`;
}

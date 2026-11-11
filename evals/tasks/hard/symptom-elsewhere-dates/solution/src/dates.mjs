const DAY_MS = 24 * 60 * 60 * 1000;

/**
 * Parses an ISO 8601 timestamp string (with Z or an offset such as +02:00) into a Date.
 * @param value - the timestamp text
 */
export function parseTimestamp(value) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) throw new Error(`invalid timestamp: ${value}`);
  return date;
}

/**
 * Returns midnight UTC at the start of the Monday of the (UTC) week that contains the given date.
 * @param date - a Date
 */
export function startOfWeek(date) {
  const daysSinceMonday = (date.getUTCDay() + 6) % 7;
  return new Date(Date.UTC(date.getUTCFullYear(), date.getUTCMonth(), date.getUTCDate() - daysSinceMonday));
}

/**
 * Returns a new Date that is the given number of 24 hour days after (or before, when negative)
 * the date. Works on UTC instants so the process time zone has no effect.
 * @param date - a Date
 * @param days - whole days to add
 */
export function addDays(date, days) {
  return new Date(date.getTime() + days * DAY_MS);
}

/**
 * Formats the UTC calendar date of a Date as YYYY-MM-DD.
 * @param date - a Date
 */
export function formatDate(date) {
  const month = String(date.getUTCMonth() + 1).padStart(2, "0");
  const day = String(date.getUTCDate()).padStart(2, "0");
  return `${date.getUTCFullYear()}-${month}-${day}`;
}

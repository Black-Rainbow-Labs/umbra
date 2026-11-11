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
 * Returns midnight at the start of the Monday of the week that contains the given date.
 * @param date - a Date
 */
export function startOfWeek(date) {
  const start = new Date(date.getTime());
  const day = start.getDay();
  start.setDate(start.getDate() - (day - 1));
  start.setHours(0, 0, 0, 0);
  return start;
}

/**
 * Returns a new Date that is the given number of days after (or before, when negative) the date.
 * @param date - a Date
 * @param days - whole days to add
 */
export function addDays(date, days) {
  const result = new Date(date.getTime());
  result.setDate(result.getDate() + days);
  return result;
}

/**
 * Formats a Date as YYYY-MM-DD.
 * @param date - a Date
 */
export function formatDate(date) {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

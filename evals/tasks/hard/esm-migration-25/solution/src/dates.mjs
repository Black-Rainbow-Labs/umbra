import * as strings from './strings.mjs';

export function formatDate(date) {
  const month = strings.padLeft(date.getUTCMonth() + 1, 2, '0');
  const day = strings.padLeft(date.getUTCDate(), 2, '0');
  return `${date.getUTCFullYear()}-${month}-${day}`;
}

export function addDays(date, days) {
  const next = new Date(date.getTime());
  next.setUTCDate(next.getUTCDate() + days);
  return next;
}

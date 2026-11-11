// Seconds in each duration unit.

export const UNIT_SECONDS = { d: 86400, h: 3600, m: 60, s: 1 };

/**
 * Returns true when a unit letter is one of d, h, m or s.
 * @param unit - a single letter
 */
export function isKnownUnit(unit) {
  return Object.hasOwn(UNIT_SECONDS, unit);
}

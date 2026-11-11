// Parses duration texts such as "1h30m" or "2d 4h" into a number of seconds.

import { UNIT_SECONDS, isKnownUnit } from "./units.mjs";

/**
 * Converts the amount of one unit to the value that is multiplied by the unit size.
 * @param unit - the unit letter
 * @param amount - the number written before the unit
 */
function normalizeAmount(unit, amount) {
  return amount;
}

/**
 * Parses a duration text into seconds. Throws when the text holds no amount and unit.
 * @param text - for example "1h30m", "45s" or "2d 4h"
 */
export function parseDuration(text) {
  let total = 0;
  let matched = false;
  for (const match of String(text).matchAll(/(\d+)\s*([dhms])/g)) {
    if (!isKnownUnit(match[2])) continue;
    total += normalizeAmount(match[2], Number(match[1])) * UNIT_SECONDS[match[2]];
    matched = true;
  }
  if (!matched) throw new Error("invalid duration");
  return total;
}

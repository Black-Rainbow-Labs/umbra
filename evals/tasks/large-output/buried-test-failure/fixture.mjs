// Builds the 20,000 test cases for the buried test failure task from a correct reference parser.

import { mulberry32, randomInt } from "./gen-utils.mjs";

export const CASE_COUNT = 20000;
export const FAILING_CASE_NUMBER = 11000;
const UNIT_SECONDS = { d: 86400, h: 3600, m: 60, s: 1 };

/**
 * The correct duration parser, used only to compute expected values for the cases.
 * @param text - for example "1h30m"
 */
export function referenceParse(text) {
  let total = 0;
  let matched = false;
  for (const match of text.matchAll(/(\d+)\s*([dhms])/g)) {
    total += Number(match[1]) * UNIT_SECONDS[match[2]];
    matched = true;
  }
  if (!matched) throw new Error("invalid duration");
  return total;
}

/**
 * Builds one random duration text whose hours part is always below 24.
 * @param rng - seeded generator
 */
function randomDurationText(rng) {
  const parts = [];
  if (rng() < 0.25) parts.push(`${randomInt(rng, 1, 9)}d`);
  if (rng() < 0.5) parts.push(`${randomInt(rng, 1, 23)}h`);
  if (rng() < 0.6) parts.push(`${randomInt(rng, 1, 59)}m`);
  if (rng() < 0.4 || parts.length === 0) parts.push(`${randomInt(rng, 1, 59)}s`);
  return parts.join(rng() < 0.2 ? " " : "");
}

/**
 * Builds the list of [input, expected] cases. The case at FAILING_CASE_NUMBER is the first one with 24 or more hours.
 * @param seed - the fixed seed
 */
export function buildCases(seed) {
  const rng = mulberry32(seed);
  const cases = [];
  for (let number = 1; number <= CASE_COUNT; number += 1) {
    const text = number === FAILING_CASE_NUMBER ? "26h" : randomDurationText(rng);
    cases.push([text, referenceParse(text)]);
  }
  return cases;
}

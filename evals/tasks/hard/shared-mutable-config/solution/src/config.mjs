import { getDefaults } from "./defaults.mjs";

/**
 * Tells whether a value is a plain object (not an array and not null).
 * @param value - any value
 */
function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

/**
 * Merges source into target: plain objects merge key by key, everything else (arrays included)
 * replaces the target value with a copy, and undefined values are skipped.
 * @param target - the object that receives the values
 * @param source - the overrides
 */
function mergeInto(target, source) {
  for (const [key, value] of Object.entries(source)) {
    if (value === undefined) continue;
    if (isPlainObject(value) && isPlainObject(target[key])) mergeInto(target[key], value);
    else target[key] = structuredClone(value);
  }
  return target;
}

/**
 * Builds the settings for one request: a fresh copy of the defaults with the overrides merged in.
 * The result shares nothing with the defaults or the overrides.
 * @param overrides - optional settings that replace the defaults
 */
export function buildConfig(overrides) {
  return mergeInto(getDefaults(), overrides ?? {});
}

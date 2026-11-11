import { getDefaults } from "./defaults.mjs";
import { deepClone, isPlainObject } from "./clone.mjs";

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
    else target[key] = deepClone(value);
  }
  return target;
}

/**
 * Returns a new config: a copy of config with the overrides merged in. Neither argument is modified or shared.
 * @param config - the settings to start from
 * @param overrides - optional settings merged on top
 */
export function mergeConfig(config, overrides) {
  return mergeInto(deepClone(config), overrides ?? {});
}

/**
 * Builds the settings for one request: a fresh copy of the defaults with the overrides merged in.
 * The result shares nothing with the defaults or the overrides.
 * @param overrides - optional settings that replace the defaults
 */
export function buildConfig(overrides) {
  return mergeConfig(getDefaults(), overrides);
}

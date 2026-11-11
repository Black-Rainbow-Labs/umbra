import { DEFAULTS } from "./defaults.mjs";

/**
 * Builds the settings for one request: the defaults with the caller's overrides on top.
 * @param overrides - optional settings that replace the defaults
 */
export function buildConfig(overrides = {}) {
  return Object.assign({}, DEFAULTS, overrides);
}

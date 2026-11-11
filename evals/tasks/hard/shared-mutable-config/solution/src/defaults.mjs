import { deepClone, deepFreeze } from "./clone.mjs";

/** The settings every request starts from. Deeply frozen so nothing can change them. */
export const DEFAULTS = deepFreeze({
  timeout: 30,
  retries: { max: 3, backoff: [100, 200, 400] },
  headers: { accept: "application/json" },
  features: { beta: false, tags: [] },
});

/**
 * Returns a fresh, unfrozen deep copy of the default settings.
 */
export function getDefaults() {
  return deepClone(DEFAULTS);
}

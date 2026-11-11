import { deepFreeze } from "./clone.mjs";
import { mergeConfig } from "./config.mjs";

/** Named bundles of settings that a request can ask for with `preset`. Deeply frozen. */
export const PRESETS = deepFreeze({
  fast: { timeout: 5, retries: { max: 2, backoff: [10, 20] } },
  slow: { timeout: 120, retries: { max: 6, backoff: [500, 1000] } },
  secure: { headers: { "x-secure": "1" }, features: { tags: ["tls"] } },
});

/**
 * Returns a copy of the config with the settings of a named preset merged on top. The argument is never modified.
 * @param config - the request settings
 * @param name - the preset name (an own name of PRESETS)
 */
export function applyPreset(config, name) {
  if (typeof name !== "string" || !Object.hasOwn(PRESETS, name)) throw new RangeError(`unknown preset: ${name}`);
  return mergeConfig(config, PRESETS[name]);
}

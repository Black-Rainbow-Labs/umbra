/** Named bundles of settings that a request can ask for with `preset`. */
export const PRESETS = {
  fast: { timeout: 5, retries: { max: 2, backoff: [10, 20] } },
  slow: { timeout: 120, retries: { max: 6, backoff: [500, 1000] } },
  secure: { headers: { "x-secure": "1" }, features: { tags: ["tls"] } },
};

/**
 * Puts the settings of a named preset on top of a config.
 * @param config - the request settings
 * @param name - the preset name
 */
export function applyPreset(config, name) {
  const preset = PRESETS[name];
  if (!preset) throw new RangeError(`unknown preset: ${name}`);
  return Object.assign(config, preset);
}

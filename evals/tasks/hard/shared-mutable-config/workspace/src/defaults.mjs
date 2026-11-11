/** The settings every request starts from. */
export const DEFAULTS = {
  timeout: 30,
  retries: { max: 3, backoff: [100, 200, 400] },
  headers: { accept: "application/json" },
  features: { beta: false, tags: [] },
};

/**
 * Returns the default settings.
 */
export function getDefaults() {
  return DEFAULTS;
}

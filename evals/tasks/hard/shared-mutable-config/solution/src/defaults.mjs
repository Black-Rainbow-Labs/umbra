/**
 * Freezes a value and everything inside it.
 * @param value - an object, array or primitive
 */
function deepFreeze(value) {
  if (value !== null && typeof value === "object") {
    Object.values(value).forEach(deepFreeze);
    Object.freeze(value);
  }
  return value;
}

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
  return structuredClone(DEFAULTS);
}

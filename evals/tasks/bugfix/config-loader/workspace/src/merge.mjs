// Merges a config over the defaults.

/**
 * True for plain objects, which are merged key by key.
 * @param value - any value
 */
function isPlainObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Returns the base object with the override values applied on top. Nested objects merge key by key.
 * @param base - the settings to start from
 * @param override - the settings that win
 */
export function deepMerge(base, override) {
  const result = base;
  for (const [key, value] of Object.entries(override)) {
    if (isPlainObject(value) && isPlainObject(result[key])) {
      result[key] = deepMerge(result[key], value);
    } else {
      result[key] = value;
    }
  }
  return result;
}

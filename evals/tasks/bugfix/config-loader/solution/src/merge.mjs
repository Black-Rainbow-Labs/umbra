// Merges a config over the defaults.

/**
 * True for plain objects, which are merged key by key.
 * @param value - any value
 */
function isPlainObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * Copies plain objects all the way down so the copy shares nothing with the original.
 * @param value - any value
 */
function cloneTree(value) {
  if (Array.isArray(value)) return value.map(cloneTree);
  if (!isPlainObject(value)) return value;
  return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, cloneTree(child)]));
}

/**
 * Returns a new object: the base settings with the override values applied on top. Neither input changes.
 * @param base - the settings to start from
 * @param override - the settings that win
 */
export function deepMerge(base, override) {
  const result = cloneTree(base);
  for (const [key, value] of Object.entries(override)) {
    if (isPlainObject(value) && isPlainObject(result[key])) {
      result[key] = deepMerge(result[key], value);
    } else {
      result[key] = cloneTree(value);
    }
  }
  return result;
}

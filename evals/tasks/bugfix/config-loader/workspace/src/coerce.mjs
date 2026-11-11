// Turns the strings read from a config file into booleans and numbers.

/**
 * Converts "true" and "false" to booleans and plain numbers to numbers. Anything else stays a string.
 * @param value - a string read from the config file
 */
export function coerceValue(value) {
  if (value === "true" || value === "false") return Boolean(value);
  if (/^-?\d+(\.\d+)?$/.test(value)) return Number(value);
  return value;
}

/**
 * Applies coerceValue to every string in a nested object and returns a new object.
 * @param tree - an object of strings and nested objects
 */
export function coerceTree(tree) {
  const result = {};
  for (const [key, value] of Object.entries(tree)) {
    result[key] = typeof value === "object" && value !== null ? coerceTree(value) : coerceValue(value);
  }
  return result;
}

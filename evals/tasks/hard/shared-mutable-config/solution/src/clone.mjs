/**
 * Tells whether a value is a plain object (not an array, not null, not a function).
 * @param value - any value
 */
export function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

/**
 * Deep copies plain objects and arrays. Every other value (numbers, strings, null, functions) is kept as it is.
 * @param value - any value
 */
export function deepClone(value) {
  if (Array.isArray(value)) return value.map(deepClone);
  if (isPlainObject(value)) return Object.fromEntries(Object.entries(value).map(([key, inner]) => [key, deepClone(inner)]));
  return value;
}

/**
 * Freezes a value and every object and array inside it.
 * @param value - an object, array or primitive
 */
export function deepFreeze(value) {
  if (value !== null && typeof value === "object") {
    Object.values(value).forEach(deepFreeze);
    Object.freeze(value);
  }
  return value;
}

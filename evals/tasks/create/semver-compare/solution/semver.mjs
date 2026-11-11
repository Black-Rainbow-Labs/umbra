// Semantic version parsing and comparison.

const VERSION_PATTERN = /^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;

/**
 * Converts one prerelease identifier to a number when it is all digits.
 * @param identifier - one dot separated prerelease identifier
 */
function toPrereleaseIdentifier(identifier) {
  if (!/^\d+$/.test(identifier)) return identifier;
  if (identifier.length > 1 && identifier.startsWith("0")) throw new Error(`Invalid version: leading zero in ${identifier}`);
  return Number(identifier);
}

/**
 * Parses a version string into its numeric parts, prerelease and build lists.
 * @param text - the version string
 */
export function parse(text) {
  if (typeof text !== "string") throw new Error("Invalid version: not a string");
  const match = VERSION_PATTERN.exec(text);
  if (!match) throw new Error(`Invalid version: ${text}`);
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4] ? match[4].split(".").map(toPrereleaseIdentifier) : [],
    build: match[5] ? match[5].split(".") : [],
  };
}

/**
 * Orders two values of the same primitive kind, returning -1, 0 or 1.
 * @param a - first value
 * @param b - second value
 */
function orderValues(a, b) {
  if (a < b) return -1;
  return a > b ? 1 : 0;
}

/**
 * Compares two prerelease identifiers by the semantic version rules.
 * @param a - first identifier
 * @param b - second identifier
 */
function comparePrereleaseIdentifier(a, b) {
  const aNumeric = typeof a === "number";
  const bNumeric = typeof b === "number";
  if (aNumeric && bNumeric) return orderValues(a, b);
  if (aNumeric) return -1;
  if (bNumeric) return 1;
  return orderValues(a, b);
}

/**
 * Compares two prerelease lists, where an empty list is higher than any non empty list.
 * @param a - first prerelease list
 * @param b - second prerelease list
 */
function comparePrerelease(a, b) {
  if (!a.length && !b.length) return 0;
  if (!a.length) return 1;
  if (!b.length) return -1;
  for (let index = 0; index < Math.min(a.length, b.length); index += 1) {
    const result = comparePrereleaseIdentifier(a[index], b[index]);
    if (result !== 0) return result;
  }
  return orderValues(a.length, b.length);
}

/**
 * Compares two version strings and returns -1, 0 or 1.
 * @param a - first version string
 * @param b - second version string
 */
export function compare(a, b) {
  const left = parse(a);
  const right = parse(b);
  for (const key of ["major", "minor", "patch"]) {
    const result = orderValues(left[key], right[key]);
    if (result !== 0) return result;
  }
  return comparePrerelease(left.prerelease, right.prerelease);
}

/**
 * Returns a new array of the versions sorted from lowest to highest precedence.
 * @param list - an array of version strings
 */
export function sortVersions(list) {
  return list.map((value, index) => ({ value, index })).sort((x, y) => compare(x.value, y.value) || x.index - y.index).map((entry) => entry.value);
}

/**
 * Groups items by the key the callback returns for each item.
 * @param items - the items to group
 * @param keyFn - a function that returns the group key for an item
 * @returns a new Map from key to the array of items with that key, in first seen key order
 */
export function groupBy(items, keyFn) {
  const groups = new Map();
  for (const item of items) {
    const key = keyFn(item);
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(item);
  }
  return groups;
}

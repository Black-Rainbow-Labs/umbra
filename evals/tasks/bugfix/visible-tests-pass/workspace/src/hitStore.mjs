// Remembers the timestamps of accepted requests for each key.

/**
 * Creates an in memory store of request timestamps keyed by client.
 */
export function createHitStore() {
  const hitsByKey = new Map();
  return {
    list: (key) => hitsByKey.get(key) ?? [],
    replace: (key, timestamps) => { hitsByKey.set(key, timestamps); },
    record: (key, timestamp) => {
      const existing = hitsByKey.get(key) ?? [];
      hitsByKey.set(key, [...existing, timestamp]);
    },
  };
}

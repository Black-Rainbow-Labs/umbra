import { deepClone } from "./clone.mjs";

/**
 * Returns a copy of the config with retries.schedule: one delay for each allowed retry, taken
 * from retries.backoff in order, repeating the last backoff value (0 when there is none) when
 * there are more retries than backoff entries. retries.backoff itself is left as it is.
 * @param config - the request settings
 */
export function planRetries(config) {
  const next = deepClone(config);
  const { backoff, max } = next.retries;
  const last = backoff[backoff.length - 1] ?? 0;
  next.retries.schedule = Array.from({ length: max }, (_, index) => backoff[index] ?? last);
  return next;
}

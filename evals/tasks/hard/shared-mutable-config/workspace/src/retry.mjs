/**
 * Adds retries.schedule: one delay for each allowed retry, taken from retries.backoff in order,
 * repeating the last backoff value when there are more retries than backoff entries.
 * @param config - the request settings
 */
export function planRetries(config) {
  const { retries } = config;
  retries.backoff.length = Math.min(retries.backoff.length, retries.max);
  const last = retries.backoff[retries.backoff.length - 1] ?? 0;
  retries.schedule = Array.from({ length: retries.max }, (_, index) => retries.backoff[index] ?? last);
  return config;
}

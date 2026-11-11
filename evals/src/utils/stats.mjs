// Small statistics helpers for the scorecard.

/**
 * Sums a list of numbers, treating missing values as 0.
 * @param values - the numbers
 */
export function sum(values) {
  return values.reduce((total, value) => total + (Number.isFinite(value) ? value : 0), 0);
}

/**
 * Returns the mean of a list of numbers, or null for an empty list.
 * @param values - the numbers
 */
export function mean(values) {
  const finite = values.filter(Number.isFinite);
  return finite.length ? sum(finite) / finite.length : null;
}

/**
 * Divides, returning null when the divisor is 0.
 * @param numerator - top
 * @param denominator - bottom
 */
export function ratio(numerator, denominator) {
  return denominator ? numerator / denominator : null;
}

/**
 * Two sided exact sign test: the probability of a split at least this uneven between wins and
 * losses if both agents were equally good. Ties are left out.
 * @param wins - tasks where the first agent did better
 * @param losses - tasks where the second agent did better
 */
export function signTest(wins, losses) {
  const n = wins + losses;
  if (n === 0) return 1;
  const k = Math.min(wins, losses);
  let tail = 0;
  for (let i = 0; i <= k; i += 1) tail += binomial(n, i);
  return Math.min(1, (2 * tail) / 2 ** n);
}

/**
 * Binomial coefficient n choose k.
 * @param n - total
 * @param k - chosen
 */
function binomial(n, k) {
  let result = 1;
  for (let i = 1; i <= k; i += 1) result = (result * (n - k + i)) / i;
  return result;
}

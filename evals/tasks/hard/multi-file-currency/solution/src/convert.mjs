import { SUPPORTED_CURRENCIES, decimalsFor } from "./money.mjs";

export const RATES_TO_USD = { USD: 1, EUR: 1.08, GBP: 1.27, JPY: 0.0067, CAD: 0.74 };

/**
 * Trims and uppercases a currency code and throws when it is not supported.
 * @param code - the currency code as given
 */
export function checkCurrency(code) {
  const upper = String(code).trim().toUpperCase();
  if (!SUPPORTED_CURRENCIES.includes(upper)) throw new Error(`unsupported currency: ${upper}`);
  return upper;
}

/**
 * Converts an amount between two supported currencies through USD and rounds the result to the decimals of the
 * target currency (cents, or whole yen for JPY).
 * @param amount - the amount in the source currency
 * @param from - the source currency code
 * @param to - the target currency code
 */
export function convert(amount, from, to) {
  const source = checkCurrency(from);
  const target = checkCurrency(to);
  const factor = 10 ** decimalsFor(target);
  return Math.round(((amount * RATES_TO_USD[source]) / RATES_TO_USD[target]) * factor) / factor;
}

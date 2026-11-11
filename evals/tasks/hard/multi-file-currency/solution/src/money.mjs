export const SUPPORTED_CURRENCIES = ["USD", "EUR", "GBP", "JPY", "CAD"];
export const DEFAULT_CURRENCY = "USD";

/**
 * Returns how many decimals amounts in a currency are written with.
 * @param currency - a currency code
 */
export function decimalsFor(currency) {
  return currency === "JPY" ? 0 : 2;
}

/**
 * Formats an amount with the decimals of its currency.
 * @param amount - the number to format
 * @param currency - a currency code, USD when missing
 */
export function formatAmount(amount, currency = DEFAULT_CURRENCY) {
  return Number(amount).toFixed(decimalsFor(currency));
}

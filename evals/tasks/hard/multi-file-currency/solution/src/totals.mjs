import { DEFAULT_CURRENCY, formatAmount } from "./money.mjs";
import { convert } from "./convert.mjs";

/**
 * Adds up quantity times price for each currency and returns [[code, total], ...] in alphabetical order of the code.
 * An item without a currency counts as USD.
 * @param items - the items to total
 */
export function totalsByCurrency(items) {
  const totals = new Map();
  for (const item of items) {
    const currency = item.currency ?? DEFAULT_CURRENCY;
    totals.set(currency, (totals.get(currency) ?? 0) + item.qty * item.price);
  }
  return [...totals.entries()].sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
}

/**
 * Converts every currency total to one currency and adds the converted values.
 * @param totals - the result of totalsByCurrency
 * @param to - the currency code to convert to
 */
export function grandTotal(totals, to) {
  return totals.reduce((sum, [code, total]) => sum + convert(total, code, to), 0);
}

/**
 * Returns the grand total formatted with the decimals of its currency.
 * @param totals - the result of totalsByCurrency
 * @param to - the currency code to convert to
 */
export function formatGrandTotal(totals, to) {
  return formatAmount(grandTotal(totals, to), to.trim().toUpperCase());
}

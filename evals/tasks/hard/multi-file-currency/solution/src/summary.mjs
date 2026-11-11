import { formatAmount } from "./money.mjs";
import { checkCurrency } from "./convert.mjs";
import { totalsByCurrency, formatGrandTotal } from "./totals.mjs";

/**
 * Renders a JSON summary: the totals per currency (numbers rounded to the currency's decimals, keys in alphabetical
 * order), the item count and, when options.totalIn is given and there are items, the grand total in that currency.
 * @param items - the items to summarize
 * @param options - { totalIn } an optional currency code
 */
export function renderSummary(items, options = {}) {
  const totals = totalsByCurrency(items);
  const out = { totals: Object.fromEntries(totals.map(([code, total]) => [code, Number(formatAmount(total, code))])), items: items.length };
  if (options.totalIn) {
    const code = checkCurrency(options.totalIn);
    if (totals.length) out.totalIn = { currency: code, amount: Number(formatGrandTotal(totals, code)) };
  }
  return JSON.stringify(out, null, 2) + "\n";
}

import { DEFAULT_CURRENCY, formatAmount } from "./money.mjs";
import { checkCurrency } from "./convert.mjs";
import { totalsByCurrency, formatGrandTotal } from "./totals.mjs";

/**
 * Renders a plain text report: one line per item, a blank line, one total line per currency in alphabetical order of
 * the code, then, when options.totalIn is given and there are items, one line with the grand total in that currency,
 * and the item count.
 * @param items - the items to report on
 * @param options - { totalIn } an optional currency code
 */
export function renderReport(items, options = {}) {
  const lines = ["Inventory report"];
  for (const item of items) {
    const currency = item.currency ?? DEFAULT_CURRENCY;
    const lineTotal = item.qty * item.price;
    lines.push(`${item.sku} | ${item.name} | ${item.qty} x ${formatAmount(item.price, currency)} ${currency} = ${formatAmount(lineTotal, currency)} ${currency}`);
  }
  lines.push("");
  const totals = totalsByCurrency(items);
  for (const [code, total] of totals) lines.push(`Total (${code}): ${formatAmount(total, code)}`);
  if (options.totalIn) {
    const code = checkCurrency(options.totalIn);
    if (totals.length) lines.push(`Total in ${code}: ${formatGrandTotal(totals, code)}`);
  }
  lines.push(`Items: ${items.length}`);
  return lines.join("\n") + "\n";
}

import { DEFAULT_CURRENCY, formatAmount } from "./money.mjs";

/**
 * Renders a plain text report: one line per item, a blank line, one total line per currency in
 * alphabetical order of the code, and the item count.
 * @param items - the items to report on
 */
export function renderReport(items) {
  const lines = ["Inventory report"];
  const totals = new Map();
  for (const item of items) {
    const currency = item.currency ?? DEFAULT_CURRENCY;
    const lineTotal = item.qty * item.price;
    totals.set(currency, (totals.get(currency) ?? 0) + lineTotal);
    lines.push(`${item.sku} | ${item.name} | ${item.qty} x ${formatAmount(item.price, currency)} ${currency} = ${formatAmount(lineTotal, currency)} ${currency}`);
  }
  lines.push("");
  for (const code of [...totals.keys()].sort()) lines.push(`Total (${code}): ${formatAmount(totals.get(code), code)}`);
  lines.push(`Items: ${items.length}`);
  return lines.join("\n") + "\n";
}

/**
 * Renders a plain text report: one line per item, a blank line, the total and the item count.
 * @param items - the items to report on
 */
export function renderReport(items) {
  const lines = ["Inventory report"];
  let total = 0;
  for (const item of items) {
    const lineTotal = item.qty * item.price;
    total += lineTotal;
    lines.push(`${item.sku} | ${item.name} | ${item.qty} x ${item.price.toFixed(2)} = ${lineTotal.toFixed(2)}`);
  }
  lines.push("");
  lines.push(`Total: ${total.toFixed(2)}`);
  lines.push(`Items: ${items.length}`);
  return lines.join("\n") + "\n";
}

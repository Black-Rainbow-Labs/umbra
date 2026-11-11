/**
 * Quotes a CSV field when it contains a comma, a quote or a line break.
 * @param value - the field value
 */
function csvField(value) {
  const text = String(value);
  return /[",\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

/**
 * Serializes items as CSV with the header sku,name,qty,price and prices with 2 decimals.
 * @param items - the items to write
 */
export function toCsv(items) {
  const lines = ["sku,name,qty,price"];
  for (const item of items) {
    lines.push([csvField(item.sku), csvField(item.name), item.qty, item.price.toFixed(2)].join(","));
  }
  return lines.join("\n") + "\n";
}

/**
 * Serializes items as pretty printed JSON with the keys in the order sku, name, qty, price.
 * @param items - the items to write
 */
export function toJson(items) {
  const plain = items.map((item) => ({ sku: item.sku, name: item.name, qty: item.qty, price: item.price }));
  return JSON.stringify(plain, null, 2) + "\n";
}

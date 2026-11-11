import { DEFAULT_CURRENCY, formatAmount } from "./money.mjs";

/**
 * Quotes a CSV field when it contains a comma, a quote or a line break.
 * @param value - the field value
 */
function csvField(value) {
  const text = String(value);
  return /[",\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
}

/**
 * Serializes items as CSV with the header sku,name,qty,price,currency. Prices use the decimals of
 * the item's currency, and an item without a currency is written as USD.
 * @param items - the items to write
 */
export function toCsv(items) {
  const lines = ["sku,name,qty,price,currency"];
  for (const item of items) {
    const currency = item.currency ?? DEFAULT_CURRENCY;
    lines.push([csvField(item.sku), csvField(item.name), item.qty, formatAmount(item.price, currency), currency].join(","));
  }
  return lines.join("\n") + "\n";
}

/**
 * Serializes items as pretty printed JSON with the keys in the order sku, name, qty, price,
 * currency. The price stays a number.
 * @param items - the items to write
 */
export function toJson(items) {
  const plain = items.map((item) => ({ sku: item.sku, name: item.name, qty: item.qty, price: item.price, currency: item.currency ?? DEFAULT_CURRENCY }));
  return JSON.stringify(plain, null, 2) + "\n";
}

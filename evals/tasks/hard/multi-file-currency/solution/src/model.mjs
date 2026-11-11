import { DEFAULT_CURRENCY } from "./money.mjs";

/**
 * Converts a raw field to a number, treating a blank field as NaN so the validator can reject it.
 * @param raw - the raw field value
 */
function toNumber(raw) {
  if (raw === undefined || raw === null || String(raw).trim() === "") return NaN;
  return Number(raw);
}

/**
 * Normalizes a raw currency field: trimmed and uppercased, or the default when blank or missing.
 * @param raw - the raw currency value
 * @param defaultCurrency - the code to use when the value is blank
 */
function normalizeCurrency(raw, defaultCurrency) {
  const text = raw === undefined || raw === null ? "" : String(raw).trim();
  return (text === "" ? defaultCurrency : text).toUpperCase();
}

/**
 * Builds an inventory item from raw field values, trimming text and converting numbers.
 * @param raw - an object with sku, name, qty, price and optional currency fields
 * @param defaultCurrency - the currency for a blank or missing currency field, USD when omitted
 */
export function createItem(raw, defaultCurrency = DEFAULT_CURRENCY) {
  return {
    sku: String(raw.sku ?? "").trim(),
    name: String(raw.name ?? "").trim(),
    qty: toNumber(raw.qty),
    price: toNumber(raw.price),
    currency: normalizeCurrency(raw.currency, defaultCurrency),
  };
}

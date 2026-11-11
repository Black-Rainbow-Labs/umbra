/**
 * Converts a raw field to a number, treating a blank field as NaN so the validator can reject it.
 * @param raw - the raw field value
 */
function toNumber(raw) {
  if (raw === undefined || raw === null || String(raw).trim() === "") return NaN;
  return Number(raw);
}

/**
 * Builds an inventory item from raw field values, trimming text and converting numbers.
 * @param raw - an object with sku, name, qty and price fields
 */
export function createItem(raw) {
  return {
    sku: String(raw.sku ?? "").trim(),
    name: String(raw.name ?? "").trim(),
    qty: toNumber(raw.qty),
    price: toNumber(raw.price),
  };
}

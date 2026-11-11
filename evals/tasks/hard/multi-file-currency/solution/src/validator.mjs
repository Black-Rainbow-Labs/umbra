import { SUPPORTED_CURRENCIES, DEFAULT_CURRENCY } from "./money.mjs";

/**
 * Checks one item and returns a list of error messages, empty when the item is valid. An item
 * without a currency is treated as USD; any other currency must be an exact supported code.
 * @param item - an item from createItem
 */
export function validateItem(item) {
  const errors = [];
  const currency = item.currency === undefined ? DEFAULT_CURRENCY : item.currency;
  if (!item.sku) errors.push("sku is required");
  else if (!/^[A-Z0-9-]+$/.test(item.sku)) errors.push("sku must use A-Z, 0-9 and -");
  if (!item.name) errors.push("name is required");
  if (!Number.isInteger(item.qty) || item.qty < 0) errors.push("qty must be a non-negative integer");
  if (!Number.isFinite(item.price) || item.price < 0) errors.push("price must be a non-negative number");
  else if (currency === "JPY" && !Number.isInteger(item.price)) errors.push("price must be a whole number for JPY");
  if (!SUPPORTED_CURRENCIES.includes(currency)) errors.push(`currency must be one of ${SUPPORTED_CURRENCIES.join(", ")}`);
  return errors;
}

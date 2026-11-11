// Helpers that clean up values before they reach the ledger.

/**
 * Normalizes a SKU so that " ab-12 " and "AB-12" are the same item.
 * @param sku - the SKU as typed by a person or read from a file
 */
export function normalizeSku(sku) {
  if (typeof sku !== "string" || sku.trim() === "") throw new Error("sku must be a non empty string");
  return sku.trim().toUpperCase();
}

/**
 * Checks that a quantity is a positive whole number and returns it.
 * @param quantity - the number of units
 */
export function requireQuantity(quantity) {
  if (!Number.isInteger(quantity) || quantity <= 0) throw new Error("quantity must be a positive integer");
  return quantity;
}

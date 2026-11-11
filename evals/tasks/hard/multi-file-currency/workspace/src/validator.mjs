/**
 * Checks one item and returns a list of error messages, empty when the item is valid.
 * @param item - an item from createItem
 */
export function validateItem(item) {
  const errors = [];
  if (!item.sku) errors.push("sku is required");
  else if (!/^[A-Z0-9-]+$/.test(item.sku)) errors.push("sku must use A-Z, 0-9 and -");
  if (!item.name) errors.push("name is required");
  if (!Number.isInteger(item.qty) || item.qty < 0) errors.push("qty must be a non-negative integer");
  if (!Number.isFinite(item.price) || item.price < 0) errors.push("price must be a non-negative number");
  return errors;
}

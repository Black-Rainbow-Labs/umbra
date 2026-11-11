// An inventory ledger: units received into stock and shipped out, per SKU.

import { normalizeSku, requireQuantity } from "./normalize.mjs";

/**
 * Creates an empty ledger.
 */
export function createLedger() {
  const balances = new Map();
  return {
    receive: (sku, quantity) => receiveUnits(balances, sku, quantity),
    ship: (sku, quantity) => shipUnits(balances, sku, quantity),
    balance: (sku) => balances.get(normalizeSku(sku)) ?? 0,
    snapshot: () => snapshotBalances(balances),
  };
}

/**
 * Adds units to a SKU and returns the new balance.
 * @param balances - the map of SKU to units on hand
 * @param sku - the SKU received
 * @param quantity - how many units arrived
 */
function receiveUnits(balances, sku, quantity) {
  const key = normalizeSku(sku);
  const next = (balances.get(key) ?? 0) + requireQuantity(quantity);
  balances.set(key, next);
  return next;
}

/**
 * Removes units from a SKU and returns the new balance. Throws when there is not enough stock.
 * @param balances - the map of SKU to units on hand
 * @param sku - the SKU shipped
 * @param quantity - how many units leave
 */
function shipUnits(balances, sku, quantity) {
  const key = normalizeSku(sku);
  const onHand = balances.get(key) ?? 0;
  if (requireQuantity(quantity) > onHand) throw new Error(`insufficient stock for ${key}: ${onHand} on hand, ${quantity} requested`);
  balances.set(key, onHand - quantity);
  return onHand - quantity;
}

/**
 * Returns a plain object of SKU to balance, ordered by SKU.
 * @param balances - the map of SKU to units on hand
 */
function snapshotBalances(balances) {
  return Object.fromEntries([...balances.entries()].sort(([left], [right]) => left.localeCompare(right)));
}

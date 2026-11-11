// Reports built from a ledger snapshot.

/**
 * Lists the SKUs whose balance is at or below the threshold, ordered by SKU.
 * @param ledger - a ledger from createLedger
 * @param threshold - the reorder level in units
 */
export function lowStock(ledger, threshold) {
  return Object.entries(ledger.snapshot()).filter(([, units]) => units <= threshold).map(([sku]) => sku);
}

/**
 * Totals all units on hand across every SKU.
 * @param ledger - a ledger from createLedger
 */
export function totalUnits(ledger) {
  return Object.values(ledger.snapshot()).reduce((sum, units) => sum + units, 0);
}
